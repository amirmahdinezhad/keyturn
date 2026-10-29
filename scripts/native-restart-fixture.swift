import AppKit

// This app has no accounts or network operations. Only native lifecycle is tested.
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(
    contentRect: NSRect(x: 0, y: 0, width: 360, height: 140),
    styleMask: [.titled, .closable], backing: .buffered, defer: false
)
window.title = "Keyturn Restart Fixture"
let label = NSTextField(labelWithString: "Independent restart test — no accounts")
label.frame = NSRect(x: 25, y: 55, width: 320, height: 25)
window.contentView!.addSubview(label)
window.center()
window.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps: true)
app.run()
