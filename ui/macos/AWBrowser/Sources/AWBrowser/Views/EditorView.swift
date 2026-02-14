import SwiftUI

struct EditorView: View {
    let pageTitle: String
    @EnvironmentObject var sessionViewModel: SessionViewModel
    @State private var originalText = ""
    @State private var modifiedText = ""
    @State private var editSummary = "AWB-RS automated edit"
    @State private var isLoading = false
    @State private var diffHtml = ""

    var body: some View {
        VStack(spacing: 0) {
            // Header with page title
            HStack {
                Image(systemName: "doc.text.fill")
                    .foregroundColor(.blue)
                Text(pageTitle)
                    .font(.headline)
                Spacer()
                if isLoading {
                    ProgressView()
                        .scaleEffect(0.7)
                }
            }
            .padding()
            .background(Color(.windowBackgroundColor))

            Divider()

            // Split view: Original | Modified
            HSplitView {
                // Original text
                VStack(alignment: .leading, spacing: 8) {
                    Text("Original")
                        .font(.subheadline)
                        .fontWeight(.semibold)
                        .padding(.horizontal)
                        .padding(.top, 8)

                    ScrollView {
                        TextEditor(text: .constant(originalText))
                            .font(.system(.body, design: .monospaced))
                            .disabled(true)
                            .opacity(0.8)
                    }
                }

                // Modified text with diff highlighting
                VStack(alignment: .leading, spacing: 8) {
                    Text("Modified")
                        .font(.subheadline)
                        .fontWeight(.semibold)
                        .padding(.horizontal)
                        .padding(.top, 8)

                    ScrollView {
                        if !diffHtml.isEmpty {
                            // In a real implementation, render HTML diff
                            // For now, show modified text
                            TextEditor(text: $modifiedText)
                                .font(.system(.body, design: .monospaced))
                        } else {
                            TextEditor(text: $modifiedText)
                                .font(.system(.body, design: .monospaced))
                        }
                    }
                }
            }

            Divider()

            // Controls
            VStack(spacing: 12) {
                HStack {
                    Text("Edit Summary:")
                        .font(.subheadline)
                    TextField("Edit summary", text: $editSummary)
                        .textFieldStyle(.roundedBorder)
                }

                HStack {
                    Button("Skip") {
                        skipPage()
                    }
                    .keyboardShortcut("s", modifiers: .command)

                    Button("Revert") {
                        modifiedText = originalText
                    }
                    .disabled(modifiedText == originalText)

                    Spacer()

                    Button("Open in Browser") {
                        openInBrowser()
                    }

                    Button("Apply Rules") {
                        applyRules()
                    }
                    .disabled(originalText.isEmpty)

                    Button("Save") {
                        savePage()
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(modifiedText == originalText || editSummary.isEmpty)
                    .keyboardShortcut(.return, modifiers: .command)
                }
            }
            .padding()
            .background(Color(.windowBackgroundColor))
        }
        .task {
            await loadPage()
        }
        .onChange(of: pageTitle) {
            Task {
                await loadPage()
            }
        }
    }

    private func loadPage() async {
        isLoading = true
        do {
            let pageInfo = try await sessionViewModel.getPage(title: pageTitle)
            originalText = pageInfo.wikitext
            modifiedText = pageInfo.wikitext
            diffHtml = ""
        } catch {
            print("Error loading page: \(error)")
        }
        isLoading = false
    }

    private func applyRules() {
        isLoading = true
        Task {
            do {
                let result = try await sessionViewModel.applyRules(content: originalText)
                modifiedText = result.newWikitext
                editSummary = result.summary
                diffHtml = result.diffHtml
            } catch {
                print("Error applying rules: \(error)")
            }
            isLoading = false
        }
    }

    private func savePage() {
        isLoading = true
        Task {
            do {
                try await sessionViewModel.savePage(
                    title: pageTitle,
                    content: modifiedText,
                    summary: editSummary
                )
                sessionViewModel.markPageAsProcessed(pageTitle)
                // Move to next page
            } catch {
                print("Error saving page: \(error)")
            }
            isLoading = false
        }
    }

    private func skipPage() {
        sessionViewModel.markPageAsProcessed(pageTitle)
        // Move to next page
    }

    private func openInBrowser() {
        // Construct wiki URL and open
        if let url = URL(string: "\(sessionViewModel.wikiUrl)/wiki/\(pageTitle.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? pageTitle)") {
            NSWorkspace.shared.open(url)
        }
    }
}

