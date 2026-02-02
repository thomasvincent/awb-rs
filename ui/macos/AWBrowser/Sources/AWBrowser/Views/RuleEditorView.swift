import SwiftUI

struct RuleEditorView: View {
    @Environment(\.dismiss) var dismiss
    @State private var rules: [RuleItem] = []
    @State private var selectedRule: RuleItem.ID?

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Find and Replace Rules")
                    .font(.title2)
                    .fontWeight(.semibold)
                Spacer()
                Button("Done") {
                    dismiss()
                }
                .keyboardShortcut(.return, modifiers: .command)
            }
            .padding()

            Divider()

            // Rules list with add/remove
            HStack(spacing: 0) {
                // List sidebar
                VStack(spacing: 0) {
                    List(selection: $selectedRule) {
                        ForEach(rules) { rule in
                            RuleListItem(rule: rule)
                                .tag(rule.id)
                        }
                        .onMove { from, to in
                            rules.move(fromOffsets: from, toOffset: to)
                        }
                    }

                    HStack {
                        Button(action: addRule) {
                            Image(systemName: "plus")
                        }
                        .buttonStyle(.borderless)

                        Button(action: removeRule) {
                            Image(systemName: "minus")
                        }
                        .buttonStyle(.borderless)
                        .disabled(selectedRule == nil)

                        Spacer()
                    }
                    .padding(8)
                    .background(Color(.windowBackgroundColor))
                }
                .frame(width: 250)

                Divider()

                // Rule detail editor
                if let ruleId = selectedRule,
                   let index = rules.firstIndex(where: { $0.id == ruleId }) {
                    RuleDetailEditor(rule: $rules[index])
                        .padding()
                } else {
                    VStack {
                        Image(systemName: "list.bullet.rectangle")
                            .font(.system(size: 48))
                            .foregroundColor(.secondary)
                        Text("Select a rule to edit")
                            .font(.headline)
                            .foregroundColor(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .frame(width: 700, height: 500)
    }

    private func addRule() {
        let newRule = RuleItem(
            find: "",
            replace: "",
            enabled: true,
            isRegex: false,
            caseSensitive: true,
            comment: ""
        )
        rules.append(newRule)
        selectedRule = newRule.id
    }

    private func removeRule() {
        if let ruleId = selectedRule,
           let index = rules.firstIndex(where: { $0.id == ruleId }) {
            rules.remove(at: index)
            selectedRule = nil
        }
    }
}

struct RuleListItem: View {
    let rule: RuleItem

    var body: some View {
        HStack {
            Image(systemName: rule.enabled ? "checkmark.circle.fill" : "circle")
                .foregroundColor(rule.enabled ? .green : .secondary)

            VStack(alignment: .leading, spacing: 2) {
                Text(rule.comment.isEmpty ? "Untitled Rule" : rule.comment)
                    .font(.body)
                Text("\(rule.find) → \(rule.replace)")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(1)
            }
        }
    }
}

struct RuleDetailEditor: View {
    @Binding var rule: RuleItem

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Toggle("Enabled", isOn: $rule.enabled)

            VStack(alignment: .leading, spacing: 4) {
                Text("Comment")
                    .font(.caption)
                    .foregroundColor(.secondary)
                TextField("Rule description", text: $rule.comment)
                    .textFieldStyle(.roundedBorder)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text("Find")
                    .font(.caption)
                    .foregroundColor(.secondary)
                TextEditor(text: $rule.find)
                    .font(.system(.body, design: .monospaced))
                    .frame(height: 80)
                    .border(Color.secondary.opacity(0.2))
            }

            VStack(alignment: .leading, spacing: 4) {
                Text("Replace")
                    .font(.caption)
                    .foregroundColor(.secondary)
                TextEditor(text: $rule.replace)
                    .font(.system(.body, design: .monospaced))
                    .frame(height: 80)
                    .border(Color.secondary.opacity(0.2))
            }

            HStack(spacing: 20) {
                Toggle("Regular Expression", isOn: $rule.isRegex)
                Toggle("Case Sensitive", isOn: $rule.caseSensitive)
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

#Preview {
    RuleEditorView()
}
