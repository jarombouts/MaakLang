//
//  ContentView.swift
//  Maak — the two-up playground (iPad, landscape).
//

import SwiftUI

private let LOGICAL_W: CGFloat = 320
private let LOGICAL_H: CGFloat = 240

struct ContentView: View {
    @StateObject private var sp = Schildpad()
    @StateObject private var bridge = EditorBridge()

    var body: some View {
        GeometryReader { geo in
            HStack(spacing: 0) {
                codePane
                    .frame(width: geo.size.width * 0.48)
                Divider().background(Color.white.opacity(0.1))
                viewport
                    .frame(maxWidth: .infinity)
            }
        }
        .background(Color(white: 0.06))
        .ignoresSafeArea(.keyboard)
        .onAppear {
            // calm cold start by default; MAAK_AUTOPLAY=1 (test/screenshot) runs the demo
            if ProcessInfo.processInfo.environment["MAAK_AUTOPLAY"] == "1" { sp.play() }
        }
    }

    // MARK: code pane

    private var codePane: some View {
        VStack(spacing: 0) {
            HStack {
                Circle().fill(Palette.color("groen")).frame(width: 12, height: 12)
                Text("schildpad").font(.system(.headline, design: .monospaced)).bold()
                Text("speeltuin").font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary)
                Spacer()
                if let l = sp.currentLine {
                    Text("regel \(l)").font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary)
                }
            }
            .padding(12)

            CodeEditor(text: $sp.program, bridge: bridge, currentLine: sp.currentLine)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(white: 0.08))

            if let err = sp.errorText {
                Text(err)
                    .font(.system(.callout, design: .monospaced))
                    .foregroundStyle(Color(white: 0.85))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
                    .background(Color(white: 0.12))
            }

            SuggestionBar(bridge: bridge)

            transport
        }
    }

    private var transport: some View {
        HStack(spacing: 12) {
            transportButton("STAP", "arrow.forward.to.line") { sp.stepOnce() }
            transportButton("SPEEL", "play.fill") { sp.play() }
            transportButton("PAUZE", "pause.fill") { sp.pause() }
            transportButton("OPNIEUW", "arrow.counterclockwise") { sp.reset() }
        }
        .padding(12)
        .background(Color(white: 0.04))
    }

    private func transportButton(_ label: String, _ icon: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 4) {
                Image(systemName: icon).font(.title2)
                Text(label).font(.system(.caption2, design: .monospaced))
            }
            .frame(maxWidth: .infinity).padding(.vertical, 12)
            .background(Color(white: 0.14)).cornerRadius(10)
        }
        .buttonStyle(.plain).foregroundStyle(.white)
    }

    // MARK: viewport (framebuffer + turtle sprites)

    private var viewport: some View {
        ZStack {
            Color(white: 0.11) // grey gutter so the canvas isn't jammed against the editor
            GeometryReader { geo in
                let scale = min(geo.size.width / LOGICAL_W, geo.size.height / LOGICAL_H)
                let dw = LOGICAL_W * scale, dh = LOGICAL_H * scale
                let ox = (geo.size.width - dw) / 2, oy = (geo.size.height - dh) / 2
                ZStack(alignment: .topLeading) {
                    // re-key on frameVersion so the CGImage rebuilds when pixels change
                    if let cg = sp.fb.makeImage() {
                        Image(decorative: cg, scale: 1)
                            .interpolation(.none)
                            .resizable()
                            .frame(width: dw, height: dh)
                            .offset(x: ox, y: oy)
                            .id(sp.frameVersion)
                    }
                    // a crisp edge so the black canvas reads as a distinct object in the grey gutter
                    Rectangle()
                        .strokeBorder(Color(white: 0.22), lineWidth: 1)
                        .frame(width: dw, height: dh)
                        .offset(x: ox, y: oy)
                    ForEach(sp.sprites) { s in
                        TurtleMarker(heading: s.heading, color: Palette.color(tintName(s.tint)))
                            .frame(width: 16, height: 16)
                            .offset(x: ox + CGFloat(s.x) * scale - 8, y: oy + CGFloat(s.y) * scale - 8)
                    }
                }
            }
            .padding(24)
        }
    }

    private func tintName(_ i: Int) -> String {
        let names = ["groen", "blauw", "geel", "rood", "oranje", "paars", "cyaan", "roze"]
        return names[i % names.count]
    }
}

/// A little turtle: a rounded body with a nose showing its heading.
struct TurtleMarker: View {
    let heading: Int
    let color: Color
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 4).fill(color)
            Triangle().fill(Color.white).frame(width: 6, height: 6).offset(x: 6)
        }
        .rotationEffect(.degrees(Double(heading)))
    }
}

struct Triangle: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.minX, y: r.minY))
        p.addLine(to: CGPoint(x: r.maxX, y: r.midY))
        p.addLine(to: CGPoint(x: r.minX, y: r.maxY))
        p.closeSubpath()
        return p
    }
}

#Preview {
    ContentView().preferredColorScheme(.dark)
}
