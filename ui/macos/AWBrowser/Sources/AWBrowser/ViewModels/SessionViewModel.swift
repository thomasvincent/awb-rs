import SwiftUI

@MainActor
class SessionViewModel: ObservableObject {
    @Published var isLoggedIn = false
    @Published var showLoginSheet = false
    @Published var wikiUrl = ""
    @Published var username = ""
    @Published var pageList: [String] = []
    @Published var processedPages: Set<String> = []
    @Published var savedCount = 0

    private var sessionHandle: SessionHandle?

    func login(wikiUrl: String, username: String, password: String) async throws {
        // Call FFI to create session
        // Note: This will call the Rust UniFFI-generated Swift bindings
        // For now, we'll simulate the call structure
        self.sessionHandle = createSession(
            wikiUrl: wikiUrl,
            username: username,
            password: password
        )

        // Attempt login
        try login(handle: sessionHandle!)

        // Update state
        self.wikiUrl = wikiUrl
        self.username = username
        self.isLoggedIn = true
    }

    func logout() {
        isLoggedIn = false
        sessionHandle = nil
        pageList = []
        processedPages = []
        savedCount = 0
        wikiUrl = ""
        username = ""
    }

    func fetchList(source: String, query: String) async {
        guard let handle = sessionHandle else { return }

        do {
            let list = try fetchList(handle: handle, source: source, query: query)
            self.pageList = list
        } catch {
            print("Error fetching list: \(error)")
        }
    }

    func getPage(title: String) async throws -> PageInfo {
        guard let handle = sessionHandle else {
            throw SessionError.notLoggedIn
        }

        return try getPage(handle: handle, title: title)
    }

    func applyRules(content: String) async throws -> TransformResult {
        guard let handle = sessionHandle else {
            throw SessionError.notLoggedIn
        }

        // Serialize current rules to JSON
        // For now, use empty rules
        let rulesJson = "{\"enabled_rules\":[]}"

        return try applyRules(handle: handle, content: content, rulesJson: rulesJson)
    }

    func savePage(title: String, content: String, summary: String) async throws {
        guard let handle = sessionHandle else {
            throw SessionError.notLoggedIn
        }

        try savePage(handle: handle, title: title, content: content, summary: summary)
        savedCount += 1
    }

    func markPageAsProcessed(_ title: String) {
        processedPages.insert(title)
    }
}

// Placeholder FFI function signatures
// These will be replaced by UniFFI-generated bindings

func createSession(wikiUrl: String, username: String, password: String) -> SessionHandle {
    // Placeholder implementation
    return SessionHandle(id: 1)
}

func login(handle: SessionHandle) throws {
    // Placeholder implementation
}

func fetchList(handle: SessionHandle, source: String, query: String) throws -> [String] {
    // Placeholder implementation
    return ["Page 1", "Page 2", "Page 3"]
}

func getPage(handle: SessionHandle, title: String) throws -> PageInfo {
    // Placeholder implementation
    return PageInfo(
        pageId: 1,
        title: title,
        revision: 100,
        timestamp: ISO8601DateFormatter().string(from: Date()),
        wikitext: "Sample wikitext content for \(title)",
        sizeBytes: 100,
        isRedirect: false
    )
}

func applyRules(handle: SessionHandle, content: String, rulesJson: String) throws -> TransformResult {
    // Placeholder implementation
    return TransformResult(
        newWikitext: content,
        rulesApplied: [],
        fixesApplied: [],
        summary: "AWB-RS automated edit",
        warnings: [],
        diffHtml: ""
    )
}

func savePage(handle: SessionHandle, title: String, content: String, summary: String) throws {
    // Placeholder implementation
}

enum SessionError: Error {
    case notLoggedIn
}
