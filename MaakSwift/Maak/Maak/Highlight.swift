import SwiftUI
import UIKit
import SchildpadFFI

/// One colour span from the core (schildpad_highlight): a token's 1-based line, character
/// column/length, its colour kind tag, and `ok` (false = lexically suspect → soft underline).
struct HighlightSpan: Decodable {
    let line: Int
    let col: Int
    let len: Int
    let kind: String
    let ok: Bool
}

/// THE colour-by-kind scheme (DESIGN_BRIEF §3, §6 / issues #49, #50). The same kind→colour
/// map drives editor highlighting AND the suggestion-bar pills, so a `verb` is the same blue
/// in the code as on its key. Kinds are classified by the core — the machine never rewrites
/// the child's text, so colour is the only signal separating a keyword from a name.
enum MaakColors {
    /// Hex (0xRRGGBB) per kind tag. `colour`-kind tokens are special-cased to their own hue.
    private static func hex(_ kind: String) -> UInt32 {
        switch kind {
        case "keyword": return 0xFF6FA5 // pink — the machine's control words
        case "verb":    return 0x4FB8F0 // sky — actions
        case "type":    return 0xC58AF0 // violet — types
        case "note":    return 0xFFD60A // gold — notes
        case "value":   return 0x6FD79B // mint — builtin values (links/rechts/stilte/presets)
        case "random":  return 0xFF9F2E // orange — the one source of chance (§5)
        case "number":  return 0xE8B04B // amber
        case "text":    return 0xB8DE7E // pale green — strings
        case "op":      return 0x9A9AA0 // grey — operators/brackets
        case "name":    return 0xF2F2F2 // near-white — the child's own words
        default:        return 0x7A7A80 // unknown / fallback — muted grey
        }
    }

    /// The colour for a kind. A `colour`-kind word tints as the actual colour it names.
    static func color(_ kind: String, word: String? = nil) -> Color {
        if kind == "colour", let w = word, Palette.isColour(w) { return Palette.color(w) }
        if kind == "colour" { return Color(rgb: 0xB0B0B0) }
        return Color(rgb: hex(kind))
    }

    static func uiColor(_ kind: String, word: String? = nil) -> UIColor { UIColor(color(kind, word: word)) }

    // ---- word classification (for the palette pills), memoised ----
    private static var cache: [String: String] = [:]

    /// The colour kind of a single word, via the core's classifier — so a pill's colour and the
    /// same token's colour in the editor always agree (one source of truth, #50).
    static func classify(_ word: String) -> String {
        if let hit = cache[word] { return hit }
        var kind = "name"
        if let ptr = word.withCString({ schildpad_highlight($0) }) {
            defer { schildpad_string_free(ptr) }
            let json = String(cString: ptr)
            if let data = json.data(using: .utf8),
               let spans = try? JSONDecoder().decode([HighlightSpan].self, from: data),
               let first = spans.first {
                kind = first.kind
            }
        }
        cache[word] = kind
        return kind
    }
}

extension Color {
    /// 0xRRGGBB (opaque).
    init(rgb: UInt32) {
        self.init(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }
}
