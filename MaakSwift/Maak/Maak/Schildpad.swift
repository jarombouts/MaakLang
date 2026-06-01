import Foundation
import Combine
import CoreGraphics
import SchildpadFFI

// MARK: - Event model (decoded from the core's flat JSON event stream)

struct RawEvent: Decodable {
    let t: String
    var line: Int?
    var fb: Int?
    var x: Int?; var y: Int?
    var col: Int?; var row: Int?
    var text: String?
    var colour: String?
    var mode: String?
    var msg: String?
}

struct SpriteSnapshot: Decodable, Identifiable {
    let id: Int
    let fb: Int
    let x: Int
    let y: Int
    let heading: Int
    let tint: Int
    let penDown: Bool
}

// MARK: - The framebuffer (a chunky RGBA pixel buffer the host rasterises)

final class Framebuffer {
    let width: Int
    let height: Int
    private var bytes: [UInt8] // RGBA8888, one byte per component — endianness-free

    init(cols: Int, rows: Int) {
        width = cols * 8
        height = rows * 8
        bytes = [UInt8](repeating: 0, count: width * height * 4)
        clear()
    }

    func clear() {
        var i = 0
        while i < bytes.count {
            bytes[i] = 0; bytes[i + 1] = 0; bytes[i + 2] = 0; bytes[i + 3] = 255 // opaque black
            i += 4
        }
    }

    /// `rgba` is 0xRRGGBBAA (host palette value); split into bytes here.
    func plot(_ x: Int, _ y: Int, _ rgba: UInt32) {
        guard x >= 0, y >= 0, x < width, y < height else { return }
        let i = (y * width + x) * 4
        bytes[i] = UInt8((rgba >> 24) & 0xFF)
        bytes[i + 1] = UInt8((rgba >> 16) & 0xFF)
        bytes[i + 2] = UInt8((rgba >> 8) & 0xFF)
        bytes[i + 3] = UInt8(rgba & 0xFF)
    }

    /// 8x8 block text: render each character as a filled cell (placeholder until the core
    /// rasterises glyphs, issue #21). Good enough to see `print` output land.
    func text(col: Int, row: Int, _ s: String, _ rgba: UInt32) {
        var cx = col
        for _ in s {
            let px = cx * 8, py = row * 8
            for dy in 1..<7 { for dx in 1..<7 { plot(px + dx, py + dy, rgba) } }
            cx += 1
        }
    }

    func makeImage() -> CGImage? {
        let cs = CGColorSpaceCreateDeviceRGB()
        let info = CGImageAlphaInfo.premultipliedLast.rawValue // RGBA byte order, default endianness
        return bytes.withUnsafeBytes { raw -> CGImage? in
            guard let base = raw.baseAddress,
                  let provider = CGDataProvider(data: Data(bytes: base, count: bytes.count) as CFData)
            else { return nil }
            return CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
                           bytesPerRow: width * 4, space: cs, bitmapInfo: CGBitmapInfo(rawValue: info),
                           provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
        }
    }
}

// MARK: - The engine wrapper (calls the C ABI, decodes events, drives the transport)

@MainActor
final class Schildpad: ObservableObject {
    enum Transport { case idle, playing, paused, done }

    @Published var program: String = """
    maak pietje schildpad
    pen blauw pietje
    herhaal 4
      vooruit 130 pietje
      draai rechts pietje
    """
    @Published private(set) var transport: Transport = .idle
    @Published private(set) var currentLine: Int? = nil
    @Published private(set) var errorText: String? = nil
    @Published private(set) var frameVersion: Int = 0 // bump to trigger a redraw
    @Published private(set) var sprites: [SpriteSnapshot] = []

    let cols = 40, rows = 30
    let fb: Framebuffer
    private var engine: OpaquePointer?
    private var timer: Timer?

    init() {
        fb = Framebuffer(cols: cols, rows: rows)
        engine = schildpad_new()
        schildpad_set_render_target(engine, UInt16(cols), UInt16(rows))
    }

    deinit { schildpad_free(engine) }

    // ---- transport ----

    func play() {
        loadAndReset()
        transport = .playing
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
    }

    func pause() {
        timer?.invalidate(); timer = nil
        if transport == .playing { transport = .paused }
    }

    func stepOnce() {
        if transport == .idle || transport == .done { loadAndReset() }
        timer?.invalidate(); timer = nil
        transport = .paused
        apply(schildpad_step(engine))
        refreshSprites()
        if schildpad_done(engine) { transport = .done }
    }

    func reset() {
        timer?.invalidate(); timer = nil
        transport = .idle
        apply(schildpad_reset(engine))
        refreshSprites()
        currentLine = nil
        errorText = nil
    }

    /// Live per-line execution: run one typed line immediately (the Enter gesture, §9).
    func runLine(_ src: String, line: Int) {
        apply(schildpad_run_line(engine, src, UInt32(line)))
        refreshSprites()
    }

    private func tick() {
        guard transport == .playing else { return }
        apply(schildpad_step(engine))
        refreshSprites()
        if schildpad_done(engine) || errorText != nil {
            timer?.invalidate(); timer = nil
            transport = errorText != nil ? .paused : .done
        }
    }

    private func loadAndReset() {
        errorText = nil
        apply(program.withCString { schildpad_load(engine, $0) })
        refreshSprites()
    }

    // ---- event application ----

    private func apply(_ jsonPtr: UnsafeMutablePointer<CChar>?) {
        guard let jsonPtr else { return }
        defer { schildpad_string_free(jsonPtr) }
        let json = String(cString: jsonPtr)
        guard let data = json.data(using: .utf8),
              let events = try? JSONDecoder().decode([RawEvent].self, from: data) else { return }
        for e in events { applyOne(e) }
        frameVersion &+= 1
    }

    private func applyOne(_ e: RawEvent) {
        switch e.t {
        case "line": currentLine = e.line
        case "clear": fb.clear()
        case "plot":
            if let x = e.x, let y = e.y { fb.plot(x, y, Palette.rgba(e.colour ?? "wit")) }
        case "text":
            if let c = e.col, let r = e.row { fb.text(col: c, row: r, e.text ?? "", Palette.rgba(e.colour ?? "wit")) }
        case "error": errorText = e.msg; currentLine = e.line
        case "done": break
        case "audio", "wrap": break // host audio synth comes later
        default: break
        }
    }

    private func refreshSprites() {
        guard let ptr = schildpad_sprites(engine) else { return }
        defer { schildpad_string_free(ptr) }
        let json = String(cString: ptr)
        if let data = json.data(using: .utf8),
           let s = try? JSONDecoder().decode([SpriteSnapshot].self, from: data) {
            sprites = s
        }
    }
}
