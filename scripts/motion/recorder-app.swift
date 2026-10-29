import AppKit
import CoreGraphics
import ScreenCaptureKit

// A named local app makes the Screen Recording permission reviewable in macOS.
@available(macOS 15.0, *)
final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var recorder: Recorder?
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 440, height: 140), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Keyturn Motion Recorder"
        let label = NSTextField(wrappingLabelWithString: "Local motion review. Captures only the Keyturn Motion QA window, without audio.\n\nScreen Recording permission is required.")
        label.frame = NSRect(x: 20, y: 20, width: 400, height: 100)
        window.contentView?.addSubview(label)
        window.center(); window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        Task { @MainActor in
            do {
                let recorder = Recorder(output: URL(fileURLWithPath: "/tmp/keyturn-motion/native-flow.mp4"))
                self.recorder = recorder
                try await recorder.start(bundleID: "com.7skylab.keyturn.motion-qa")
                label.stringValue = "Recording only Keyturn Motion QA, without audio.\n\nThe recording will stop when the review finishes."
                while !FileManager.default.fileExists(atPath: "/tmp/keyturn-motion/stop-recording") && !recorder.finished { try await Task.sleep(for: .milliseconds(200)) }
                await recorder.stop()
                label.stringValue = "Recording finished. Video and frame metadata saved locally."
            } catch { label.stringValue = "Screen Recording permission is required.\n\n\(error.localizedDescription)" }
        }
    }
}
@main struct Main {
  static func main() {
    guard #available(macOS 15.0, *) else { return }
    let app = NSApplication.shared
    let delegate = Delegate()
    app.delegate = delegate
    app.setActivationPolicy(.regular)
    app.run()
  }
}
