import Foundation
import AVFoundation

/// One note in a sequence (decoded from the core's audio event). `hz == nil` is a rest (stilte).
struct SynthVoice {
    let hz: Float?
    let beats: Double   // duration in beats; `do2` = 2.0, `do/4` = 0.25 (§13)
    let osc: String
    let env: String
}

/// A small, dependency-free synth for the iPad host (issue #52). The CORE is silent — it only
/// emits a declarative `AudioCmd::Sequence` (pitch already resolved to Hz for determinism,
/// LANGUAGE.md §13); this turns that into sound. Oscillator + envelope NAMES come from the
/// language; the waveform math and ADSR numbers are the host's/agency's (LANGUAGE.md §10).
@MainActor
final class Synth {
    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    // Mono float format, pinned on the player→mixer connection so a mono buffer never hits a
    // stereo output (channelCount mismatch = a hard AVFoundation crash). The mixer up-converts.
    private let format = AVAudioFormat(standardFormatWithSampleRate: 44_100, channels: 1)!
    private var sampleRate: Double { format.sampleRate }
    private var started = false

    init() {
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: format)
    }

    /// Lazily start the engine on first sound, so a silent program never grabs the audio session.
    private func ensureStarted() {
        guard !started else { return }
        do {
            #if os(iOS)
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playback, mode: .default)
            try session.setActive(true)
            #endif
            try engine.start()
            player.play()
            started = true
        } catch {
            print("synth start failed: \(error)")
        }
    }

    /// Render a sequence to one mono buffer (notes played one after another) and queue it.
    func play(_ voices: [SynthVoice], tempoBPM: Int) {
        ensureStarted()
        guard started, !voices.isEmpty else { return }

        let beat = 60.0 / Double(max(tempoBPM, 1))
        var samples: [Float] = []
        for v in voices {
            let dur = beat * max(v.beats, 0)
            let n = Int(dur * sampleRate)
            guard n > 0 else { continue }
            if let hz = v.hz, hz > 0 {
                let env = adsr(for: v.env, dur: dur)
                samples.reserveCapacity(samples.count + n)
                for i in 0..<n {
                    let t = Double(i) / sampleRate
                    let phase = (Double(hz) * t).truncatingRemainder(dividingBy: 1.0)
                    let amp = envelope(t: t, dur: dur, env)
                    samples.append(Float(wave(v.osc, phase) * amp) * 0.25)
                }
            } else {
                samples.append(contentsOf: repeatElement(0, count: n)) // stilte
            }
        }
        guard !samples.isEmpty,
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count))
        else { return }

        buffer.frameLength = AVAudioFrameCount(samples.count)
        let channel = buffer.floatChannelData![0]
        samples.withUnsafeBufferPointer { src in
            channel.update(from: src.baseAddress!, count: samples.count)
        }
        // one play at a time (#52): .interrupts cancels any in-flight buffer rather than
        // queueing it — so `play` in a loop never builds a minutes-long backlog.
        player.scheduleBuffer(buffer, at: nil, options: .interrupts, completionHandler: nil)
        if !player.isPlaying { player.play() }
    }

    func stop() {
        player.stop()
    }

    // ---- waveforms (phase in [0,1)) ----
    private func wave(_ osc: String, _ p: Double) -> Double {
        switch osc {
        case "blok":     return p < 0.5 ? 1 : -1
        case "zaag":     return 2 * p - 1
        case "driehoek": return 1 - 4 * abs(p - 0.5)
        default:         return sin(2 * .pi * p) // sinus
        }
    }

    // ---- envelopes (attack/decay in seconds, sustain level 0..1, release in seconds) ----
    private struct ADSR { let a: Double; let d: Double; let s: Double; let r: Double }

    private func adsr(for env: String, dur: Double) -> ADSR {
        switch env {
        case "langzaam-aan": return ADSR(a: min(0.3 * dur, dur * 0.5), d: 0.05, s: 0.85, r: min(0.15, dur * 0.3))
        case "vast":         return ADSR(a: 0.005, d: 0.02, s: 1.0, r: min(0.04, dur * 0.2))
        default:             return ADSR(a: 0.005, d: 0.08, s: 0.25, r: min(0.05, dur * 0.3)) // kort/plucky
        }
    }

    private func envelope(t: Double, dur: Double, _ e: ADSR) -> Double {
        let releaseStart = dur - e.r
        if t < e.a { return e.a > 0 ? t / e.a : 1 }                    // attack 0→1
        if t < e.a + e.d { return 1 - (1 - e.s) * ((t - e.a) / e.d) }  // decay 1→s
        if t < releaseStart { return e.s }                            // sustain
        let rt = (t - releaseStart) / max(e.r, 1e-4)                  // release s→0
        return max(0, e.s * (1 - rt))
    }
}
