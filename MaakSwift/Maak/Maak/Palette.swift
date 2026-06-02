import SwiftUI

/// The named Dutch colours. The NAMES are the language's; these punchy retro VALUES are the
/// host's/agency's (LANGUAGE.md §10). Stored as 0xRRGGBBAA for the framebuffer.
enum Palette {
    static let background: UInt32 = 0x0000_00FF

    private static let table: [String: UInt32] = [
        "rood":   0xFF3B30FF,
        "groen":  0x32D74BFF,
        "blauw":  0x0A84FFFF,
        "geel":   0xFFD60AFF,
        "wit":    0xFFFFFFFF,
        "zwart":  0x000000FF,
        "oranje": 0xFF9F0AFF,
        "paars":  0xBF5AF2FF,
        "cyaan":  0x64D2FFFF,
        "roze":   0xFF6482FF,
    ]

    static func rgba(_ name: String) -> UInt32 { table[name] ?? 0xFFFFFFFF }

    static func isColour(_ name: String) -> Bool { table[name] != nil }

    static func color(_ name: String) -> Color {
        let v = rgba(name)
        return Color(
            red: Double((v >> 24) & 0xFF) / 255,
            green: Double((v >> 16) & 0xFF) / 255,
            blue: Double((v >> 8) & 0xFF) / 255
        )
    }
}
