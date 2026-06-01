//
//  Item.swift
//  Maak
//
//  Created by Jeroen Rombouts on 01/06/2026.
//

import Foundation
import SwiftData

@Model
final class Item {
    var timestamp: Date
    
    init(timestamp: Date) {
        self.timestamp = timestamp
    }
}
