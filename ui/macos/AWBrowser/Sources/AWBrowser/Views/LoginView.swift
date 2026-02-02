import SwiftUI

struct LoginView: View {
    @EnvironmentObject var sessionViewModel: SessionViewModel
    @State private var wikiUrl = "https://en.wikipedia.org"
    @State private var username = ""
    @State private var password = ""
    @State private var isLoading = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "network")
                .font(.system(size: 60))
                .foregroundColor(.blue)

            Text("AutoWikiBrowser for macOS")
                .font(.title)
                .fontWeight(.bold)

            Text("Connect to your wiki")
                .font(.subheadline)
                .foregroundColor(.secondary)

            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Wiki URL")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    TextField("https://en.wikipedia.org", text: $wikiUrl)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Username")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    TextField("Wiki username", text: $username)
                        .textFieldStyle(.roundedBorder)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Password")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    SecureField("Password", text: $password)
                        .textFieldStyle(.roundedBorder)
                }
            }
            .frame(width: 300)

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundColor(.red)
                    .frame(width: 300)
            }

            HStack {
                Button("Cancel") {
                    // Clear fields
                    username = ""
                    password = ""
                    errorMessage = nil
                }
                .disabled(isLoading)

                Button("Login") {
                    login()
                }
                .buttonStyle(.borderedProminent)
                .disabled(isLoading || username.isEmpty || password.isEmpty)
            }

            if isLoading {
                ProgressView()
                    .scaleEffect(0.8)
            }
        }
        .padding(40)
        .frame(width: 400, height: 500)
    }

    private func login() {
        isLoading = true
        errorMessage = nil

        Task {
            do {
                try await sessionViewModel.login(
                    wikiUrl: wikiUrl,
                    username: username,
                    password: password
                )
            } catch {
                errorMessage = error.localizedDescription
            }
            isLoading = false
        }
    }
}

#Preview {
    LoginView()
        .environmentObject(SessionViewModel())
}
