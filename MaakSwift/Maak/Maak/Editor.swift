import SwiftUI
import UIKit
import Combine
import SchildpadFFI

/// Lets the on-screen palette insert text at the editor's cursor.
final class EditorBridge: ObservableObject {
    weak var textView: UITextView?

    func insert(_ s: String) {
        guard let tv = textView else { return }
        if let r = tv.selectedTextRange {
            tv.replace(r, withText: s)
        } else {
            tv.insertText(s)
        }
        tv.becomeFirstResponder()
    }

    func backspace() {
        textView?.deleteBackward()
    }
}

/// A real UITextView so we get a cursor the palette can insert at (a plain SwiftUI TextEditor
/// gives no cursor access). Monospaced, no autocorrect/smart-quotes — a child types real text.
/// Live syntax highlighting (#49) and a current-line band (#51) layer on top.
struct CodeEditor: UIViewRepresentable {
    @Binding var text: String
    let bridge: EditorBridge
    var currentLine: Int? = nil

    private static let baseFont = UIFont.monospacedSystemFont(ofSize: 22, weight: .regular)

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.font = Self.baseFont
        tv.backgroundColor = UIColor(white: 0.08, alpha: 1)
        tv.textColor = .white
        tv.tintColor = UIColor(red: 0.2, green: 0.85, blue: 0.4, alpha: 1)
        tv.autocapitalizationType = .none
        tv.autocorrectionType = .no
        tv.smartQuotesType = .no
        tv.smartDashesType = .no
        tv.spellCheckingType = .no
        tv.keyboardType = .asciiCapable
        tv.textContainerInset = UIEdgeInsets(top: 12, left: 8, bottom: 12, right: 8)
        tv.delegate = context.coordinator
        // current-line band, translucent so the glyphs stay readable (joint attention, #51)
        let band = context.coordinator.band
        band.backgroundColor = UIColor(red: 0.30, green: 0.85, blue: 0.45, alpha: 0.14)
        band.layer.cornerRadius = 4
        band.isHidden = true
        tv.addSubview(band)
        tv.text = text
        Self.renderHighlight(tv)
        bridge.textView = tv
        return tv
    }

    func updateUIView(_ tv: UITextView, context: Context) {
        if tv.text != text {
            tv.text = text
            Self.renderHighlight(tv)
        }
        positionBand(tv, band: context.coordinator.band)
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UITextViewDelegate {
        let parent: CodeEditor
        let band = UIView()
        init(_ p: CodeEditor) { parent = p }
        func textViewDidChange(_ tv: UITextView) {
            parent.text = tv.text
            CodeEditor.renderHighlight(tv)
        }
    }

    // ---- syntax highlighting (#49): per-token colours from the core's classifier ----
    static func renderHighlight(_ tv: UITextView) {
        let sel = tv.selectedRange
        tv.attributedText = attributed(tv.text ?? "")
        tv.selectedRange = sel
        tv.typingAttributes = [.font: baseFont, .foregroundColor: UIColor.white]
    }

    static func attributed(_ text: String) -> NSAttributedString {
        let out = NSMutableAttributedString(string: text, attributes: [
            .font: baseFont,
            .foregroundColor: UIColor.white,
        ])
        // utf16 start offset of each line (spans are per-line, columns in characters)
        let lines = text.components(separatedBy: "\n")
        var lineStart = [Int](); var acc = 0
        for ln in lines { lineStart.append(acc); acc += (ln as NSString).length + 1 }

        guard let ptr = text.withCString({ schildpad_highlight($0) }) else { return out }
        defer { schildpad_string_free(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8),
              let spans = try? JSONDecoder().decode([HighlightSpan].self, from: data) else { return out }

        for s in spans {
            let li = s.line - 1
            guard li >= 0, li < lines.count else { continue }
            let lineNS = lines[li] as NSString
            let start = min(s.col, lineNS.length)
            let length = min(s.len, lineNS.length - start)
            guard length > 0 else { continue }
            let range = NSRange(location: lineStart[li] + start, length: length)
            let word = lineNS.substring(with: NSRange(location: start, length: length))
            out.addAttribute(.foregroundColor, value: MaakColors.uiColor(s.kind, word: word), range: range)
            if !s.ok { // soft, non-alarming mark of a lexically suspect token
                out.addAttribute(.underlineStyle, value: NSUnderlineStyle.thick.rawValue, range: range)
                out.addAttribute(.underlineColor, value: UIColor(red: 1, green: 0.7, blue: 0.3, alpha: 0.8), range: range)
            }
        }
        return out
    }

    // ---- current-line band (#51) ----
    private func positionBand(_ tv: UITextView, band: UIView) {
        guard let line = currentLine, let rect = Self.lineRect(tv, line: line) else {
            band.isHidden = true
            return
        }
        band.isHidden = false
        band.frame = rect
    }

    static func lineRect(_ tv: UITextView, line: Int) -> CGRect? {
        let text = tv.text ?? ""
        let lines = text.components(separatedBy: "\n")
        let li = line - 1
        guard li >= 0, li < lines.count else { return nil }
        var acc = 0
        for i in 0..<li { acc += (lines[i] as NSString).length + 1 }
        let lineNS = lines[li] as NSString
        let charRange = NSRange(location: acc, length: lineNS.length)
        let glyphRange = tv.layoutManager.glyphRange(forCharacterRange: charRange, actualCharacterRange: nil)
        var rect = tv.layoutManager.boundingRect(forGlyphRange: glyphRange, in: tv.textContainer)
        if rect.height < 1 { rect.size.height = baseFont.lineHeight } // empty line still gets a band
        rect.origin.x = 0
        rect.size.width = tv.bounds.width
        return rect.offsetBy(dx: tv.textContainerInset.left, dy: tv.textContainerInset.top)
    }
}

/// Recently-used words, persisted, newest first. (The seed of the §2.1 "what fits next"
/// help; true context-awareness arrives with core::introspect, #28.)
final class RecentWords: ObservableObject {
    @Published var list: [String]
    private let key = "maak.recentWords"
    init() { list = UserDefaults.standard.stringArray(forKey: key) ?? [] }
    func bump(_ w: String) {
        list.removeAll { $0 == w }
        list.insert(w, at: 0)
        if list.count > 16 { list.removeLast() }
        UserDefaults.standard.set(list, forKey: key)
    }
}

/// The suggestion bar (DESIGN_BRIEF §6): ONE scrollable row of recently-used + common words,
/// capped at 10, then a ••• pill that opens the full catalogue. Inserts at the cursor — a
/// scaffold, never a replacement for typing. Digits/symbols are omitted: the system keyboard
/// already has them. The word lists should later be generated from vocab.ron (#30).
struct SuggestionBar: View {
    let bridge: EditorBridge
    @StateObject private var recents = RecentWords()
    @State private var showAll = false

    static let groups: [(String, [String])] = [
        ("werkwoorden", ["maak", "vooruit", "achteruit", "draai", "pen", "penomhoog", "penomlaag", "print", "herhaal", "doe", "keer", "play", "als", "anders", "wrapmode"]),
        ("woorden", ["links", "rechts", "random", "stilte"]),
        ("types", ["schildpad", "getal", "draairichting", "toon", "deuntje"]),
        ("kleuren", ["rood", "groen", "blauw", "geel", "wit", "zwart", "oranje", "paars", "cyaan", "roze"]),
        ("klanken", ["sinus", "blok", "zaag", "driehoek", "do", "re", "mi", "fa", "sol", "la", "si"]),
    ]
    static let allFlat: [String] = groups.flatMap { $0.1 }
    static let defaults = ["maak", "vooruit", "draai", "pen", "herhaal", "links", "rechts", "schildpad"]

    private var displayed: [String] {
        var seen = Set<String>(); var out: [String] = []
        for w in recents.list + Self.defaults + Self.allFlat {
            if seen.insert(w).inserted { out.append(w) }
            if out.count == 10 { break }
        }
        return out
    }

    private func tap(_ w: String) { bridge.insert(w + " "); recents.bump(w) }

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(displayed, id: \.self) { w in WordPill(word: w) { tap(w) } }
                Button { showAll = true } label: {
                    Text("•••").font(.system(.body, design: .monospaced))
                        .frame(minWidth: 48, minHeight: 40)
                        .background(Color(white: 0.24)).cornerRadius(9)
                }
                .buttonStyle(.plain).foregroundStyle(.white)
            }
            .padding(.horizontal, 14).padding(.vertical, 8)
        }
        .background(Color(white: 0.04))
        .sheet(isPresented: $showAll) {
            AllWordsSheet { tap($0); showAll = false }
        }
    }
}

private struct WordPill: View {
    let word: String
    let action: () -> Void
    var body: some View {
        // same colour-by-kind scheme as the editor (#50): a pill matches its token's colour.
        let kind = MaakColors.classify(word)
        let tint = MaakColors.color(kind, word: word)
        let opacity: Double = kind == "colour" ? 0.55 : (kind == "name" ? 0.16 : 0.28)
        Button(action: action) {
            Text(word)
                .font(.system(.body, design: .monospaced))
                .padding(.horizontal, 14).frame(minHeight: 40)
                .background(tint.opacity(opacity))
                .cornerRadius(9)
        }
        .buttonStyle(.plain).foregroundStyle(.white)
    }
}

/// The full catalogue, grouped and scrollable — handles a great many items gracefully.
struct AllWordsSheet: View {
    let pick: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    private let cols = [GridItem(.adaptive(minimum: 100), spacing: 8)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    ForEach(SuggestionBar.groups, id: \.0) { title, words in
                        VStack(alignment: .leading, spacing: 8) {
                            Text(title).font(.system(.headline, design: .monospaced)).foregroundStyle(.secondary)
                            LazyVGrid(columns: cols, alignment: .leading, spacing: 8) {
                                ForEach(words, id: \.self) { w in WordPill(word: w) { pick(w) } }
                            }
                        }
                    }
                }
                .padding(16)
            }
            .navigationTitle("kies iets")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("klaar") { dismiss() } } }
        }
        .preferredColorScheme(.dark)
    }
}
