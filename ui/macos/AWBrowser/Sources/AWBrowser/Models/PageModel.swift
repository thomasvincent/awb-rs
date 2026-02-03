import Foundation

// Note: SessionHandle, PageInfo, and TransformResult are now defined in Generated/awb_ffi.swift
// These models are for UI-layer data structures only

struct RuleItem: Identifiable, Codable {
    let id: UUID
    var find: String
    var replace: String
    var enabled: Bool
    var isRegex: Bool
    var caseSensitive: Bool
    var comment: String

    init(id: UUID = UUID(), find: String, replace: String, enabled: Bool, isRegex: Bool, caseSensitive: Bool, comment: String) {
        self.id = id
        self.find = find
        self.replace = replace
        self.enabled = enabled
        self.isRegex = isRegex
        self.caseSensitive = caseSensitive
        self.comment = comment
    }
}

struct RuleSet: Codable {
    var rules: [RuleItem]
    var enabledRules: [UUID]

    init(rules: [RuleItem] = []) {
        self.rules = rules
        self.enabledRules = rules.filter { $0.enabled }.map { $0.id }
    }

    func toJson() -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .prettyPrinted
        if let data = try? encoder.encode(self),
           let json = String(data: data, encoding: .utf8) {
            return json
        }
        return "{}"
    }
}
