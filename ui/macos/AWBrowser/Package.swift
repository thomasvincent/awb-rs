// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "AWBrowser",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(
            name: "AWBrowser",
            targets: ["AWBrowser"]
        )
    ],
    dependencies: [],
    targets: [
        .executableTarget(
            name: "AWBrowser",
            dependencies: [],
            path: "Sources/AWBrowser",
            linkerSettings: [
                .linkedLibrary("awb_ffi", .when(platforms: [.macOS]))
            ]
        )
    ]
)
