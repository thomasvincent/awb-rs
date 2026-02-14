import SwiftUI

struct MainView: View {
    @EnvironmentObject var sessionViewModel: SessionViewModel
    @State private var selectedPage: String?
    @State private var showRuleEditor = false

    var body: some View {
        NavigationSplitView {
            // Sidebar - Page List
            PageListSidebar(selectedPage: $selectedPage)
        } detail: {
            // Detail - Editor View
            if let page = selectedPage {
                EditorView(pageTitle: page)
            } else {
                VStack {
                    Image(systemName: "doc.text")
                        .font(.system(size: 48))
                        .foregroundColor(.secondary)
                    Text("Select a page to edit")
                        .font(.title2)
                        .foregroundColor(.secondary)
                }
            }
        }
        .navigationTitle("AWBrowser")
        .toolbar {
            ToolbarItemGroup(placement: .automatic) {
                Button(action: { showRuleEditor.toggle() }) {
                    Label("Rules", systemImage: "list.bullet.rectangle")
                }

                Button(action: { sessionViewModel.logout() }) {
                    Label("Logout", systemImage: "rectangle.portrait.and.arrow.right")
                }
            }
        }
        .sheet(isPresented: $showRuleEditor) {
            RuleEditorView()
        }
    }
}

struct PageListSidebar: View {
    @EnvironmentObject var sessionViewModel: SessionViewModel
    @Binding var selectedPage: String?
    @State private var searchQuery = ""
    @State private var listSource = "Category"
    @State private var isLoading = false

    var body: some View {
        VStack(spacing: 0) {
            // Search/Filter controls
            VStack(spacing: 8) {
                Picker("Source", selection: $listSource) {
                    Text("Category").tag("Category")
                    Text("Transclusions").tag("Transclusions")
                    Text("Links").tag("Links")
                    Text("User List").tag("UserList")
                }
                .pickerStyle(.segmented)

                HStack {
                    TextField("Search or query...", text: $searchQuery)
                        .textFieldStyle(.roundedBorder)

                    Button("Load") {
                        loadList()
                    }
                    .disabled(searchQuery.isEmpty || isLoading)
                }
            }
            .padding()

            Divider()

            // Page list
            List(sessionViewModel.pageList, id: \.self, selection: $selectedPage) { page in
                HStack {
                    Image(systemName: "doc.text")
                        .foregroundColor(.blue)
                    Text(page)
                    Spacer()
                    if sessionViewModel.processedPages.contains(page) {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                    }
                }
            }

            // Stats footer
            VStack(spacing: 4) {
                Divider()
                HStack {
                    Text("Total: \(sessionViewModel.pageList.count)")
                    Spacer()
                    Text("Processed: \(sessionViewModel.processedPages.count)")
                    Spacer()
                    Text("Saved: \(sessionViewModel.savedCount)")
                }
                .font(.caption)
                .foregroundColor(.secondary)
                .padding(.horizontal)
                .padding(.vertical, 8)
            }
        }
        .frame(minWidth: 250)
    }

    private func loadList() {
        isLoading = true
        Task {
            await sessionViewModel.fetchList(source: listSource, query: searchQuery)
            isLoading = false
        }
    }
}

