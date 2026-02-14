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
        // This calls the Rust UniFFI-generated Swift bindings
        let handle = try AWBrowser.createSession(
            wikiUrl: wikiUrl,
            username: username,
            password: password
        )

        self.sessionHandle = handle

        // Attempt login
        try AWBrowser.login(handle: handle)

        // Update state
        self.wikiUrl = wikiUrl
        self.username = username
        self.isLoggedIn = true
    }

    func logout() {
        // Destroy the session on the Rust side
        if let handle = sessionHandle {
            try? AWBrowser.destroySession(handle: handle)
        }

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
            let list = try AWBrowser.fetchList(handle: handle, source: source, query: query)
            self.pageList = list
        } catch {
            print("Error fetching list: \(error)")
        }
    }

    func getPage(title: String) async throws -> PageInfo {
        guard let handle = sessionHandle else {
            throw SessionError.notLoggedIn
        }

        return try AWBrowser.getPage(handle: handle, title: title)
    }

    func applyRules(content: String, rulesJson: String = "{\"enabled_rules\":[]}") async throws -> TransformResult {
        guard let handle = sessionHandle else {
            throw SessionError.notLoggedIn
        }

        return try AWBrowser.applyRules(handle: handle, content: content, rulesJson: rulesJson)
    }

    func savePage(title: String, content: String, summary: String) async throws {
        guard let handle = sessionHandle else {
            throw SessionError.notLoggedIn
        }

        try AWBrowser.savePage(handle: handle, title: title, content: content, summary: summary)
        savedCount += 1
    }

    func markPageAsProcessed(_ title: String) {
        processedPages.insert(title)
    }
}

enum SessionError: Error {
    case notLoggedIn
}
