import Foundation

// Models matching the FFI interface

struct SessionHandle: Codable {
    let id: UInt64
}

struct PageInfo: Codable {
    let pageId: UInt64
    let title: String
    let revision: UInt64
    let timestamp: String
    let wikitext: String
    let sizeBytes: UInt64
    let isRedirect: Bool
}

struct TransformResult: Codable {
    let newWikitext: String
    let rulesApplied: [String]
    let fixesApplied: [String]
    let summary: String
    let warnings: [String]
    let diffHtml: String
}

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

enum FfiError: Error, LocalizedError {
    case networkError(String)
    case authenticationError
    case notFound
    case permissionDenied
    case parseError(String)
    case internalError(String)

    var errorDescription: String? {
        switch self {
        case .networkError(let msg):
            return "Network error: \(msg)"
        case .authenticationError:
            return "Authentication failed"
        case .notFound:
            return "Resource not found"
        case .permissionDenied:
            return "Permission denied"
        case .parseError(let msg):
            return "Parse error: \(msg)"
        case .internalError(let msg):
            return "Internal error: \(msg)"
        }
    }
}
