import SwiftUI
import UIKit
import Combine

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
struct CodeEditor: UIViewRepresentable {
    @Binding var text: String
    let bridge: EditorBridge

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.font = .monospacedSystemFont(ofSize: 22, weight: .regular)
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
        tv.text = text
        bridge.textView = tv
        return tv
    }

    func updateUIView(_ tv: UITextView, context: Context) {
        if tv.text != text { tv.text = text }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UITextViewDelegate {
        let parent: CodeEditor
        init(_ p: CodeEditor) { parent = p }
        func textViewDidChange(_ tv: UITextView) { parent.text = tv.text }
    }
}

/// The chunky on-screen key palette (DESIGN_BRIEF §6): the verbs/keywords/colours/symbols a
/// child reaches for, as big tappable keys that insert at the cursor — a scaffold, never a
/// replacement for typing. (The word lists should later be generated from vocab.ron, issue #30.)
struct KeyboardPalette: View {
    let bridge: EditorBridge

    private let verbs = ["maak", "vooruit", "draai", "pen", "herhaal", "print", "play", "penomhoog", "penomlaag", "achteruit", "doe", "als"]
    private let words = ["links", "rechts", "random", "stilte"]
    private let types = ["schildpad", "getal", "draairichting", "toon", "deuntje"]
    private let colours = ["rood", "groen", "blauw", "geel", "wit", "oranje", "paars", "cyaan", "roze", "zwart"]
    private let symbols = ["=", "\"", "(", ")", "+", "-", "*", "/"]
    private let digits = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            row(verbs) { key($0, tint: Color(white: 0.20)) }
            row(words + types) { key($0, tint: Color(white: 0.16)) }
            row(colours) { key($0, tint: Palette.color($0).opacity(0.45)) }
            HStack(spacing: 6) {
                ForEach(symbols, id: \.self) { key($0, mono: true) }
                ForEach(digits, id: \.self) { key($0, mono: true) }
                Button(action: { bridge.backspace() }) {
                    Image(systemName: "delete.left").frame(width: 38, height: 38)
                        .background(Color(white: 0.18)).cornerRadius(8)
                }.buttonStyle(.plain).foregroundStyle(.white)
            }
        }
        .padding(8)
        .background(Color(white: 0.04))
    }

    private func row<V: View>(_ items: [String], @ViewBuilder _ make: @escaping (String) -> V) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) { ForEach(items, id: \.self) { make($0) } }
        }
    }

    private func key(_ s: String, tint: Color = Color(white: 0.16), mono: Bool = false) -> some View {
        Button(action: { bridge.insert(mono ? s : s + " ") }) {
            Text(s)
                .font(.system(mono ? .title3 : .body, design: .monospaced))
                .padding(.horizontal, mono ? 0 : 12)
                .frame(minWidth: mono ? 38 : 44, minHeight: 38)
                .background(tint)
                .cornerRadius(8)
        }
        .buttonStyle(.plain)
        .foregroundStyle(.white)
    }
}
