// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "AWBrowser",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(
            name: "AWBrowser",
            targets: ["AWBrowser"]
        )
    ],
    dependencies: [],
    targets: [
        .systemLibrary(
            name: "AwbFfiC",
            path: "Sources/AwbFfiC"
        ),
        .executableTarget(
            name: "AWBrowser",
            dependencies: ["AwbFfiC"],
            path: "Sources/AWBrowser",
            linkerSettings: [
                .unsafeFlags([
                    "-L../../../target/debug",
                    "-L../../../target/release"
                ]),
                .linkedLibrary("awb_ffi")
            ]
        )
    ]
)
