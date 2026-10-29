import Foundation
import ScreenCaptureKit
import AVFoundation
import CoreGraphics
import AppKit

// Capture only the named QA app window; no desktop, microphone or system audio.
@available(macOS 15.0, *)
final class Recorder: NSObject, SCStreamOutput, SCStreamDelegate, SCRecordingOutputDelegate {
    var stream: SCStream?
    var frames: [[String: Any]] = []
    var finished = false
    let output: URL
    let startedAt = Date().timeIntervalSince1970
    init(output: URL) { self.output = output }

    func start(bundleID: String) async throws {
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: false)
        guard let window = content.windows.first(where: { $0.owningApplication?.bundleIdentifier == bundleID && $0.frame.width >= 380 && $0.frame.height >= 400 }) else {
            throw NSError(domain: "KeyturnMotion", code: 1, userInfo: [NSLocalizedDescriptionKey: "QA window not found"])
        }
        let config = SCStreamConfiguration()
        config.width = 860
        config.height = 1200
        config.minimumFrameInterval = CMTime(value: 1, timescale: 60)
        config.queueDepth = 8
        config.showsCursor = true
        config.capturesAudio = false
        config.ignoreShadowsSingleWindow = true
        config.captureResolution = .best
        let stream = SCStream(filter: SCContentFilter(desktopIndependentWindow: window), configuration: config, delegate: self)
        self.stream = stream
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: DispatchQueue(label: "keyturn.motion.frames"))
        let recording = SCRecordingOutputConfiguration()
        recording.outputURL = output
        recording.outputFileType = .mp4
        recording.videoCodecType = .h264
        try stream.addRecordingOutput(SCRecordingOutput(configuration: recording, delegate: self))
        try await stream.startCapture()
        print("CAPTURING window=\(window.windowID) size=860x1200 target=60fps audio=false output=\(output.path)")
        fflush(stdout)
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, sampleBuffer.isValid else { return }
        let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]]
        frames.append(["pts": CMTimeGetSeconds(sampleBuffer.presentationTimeStamp), "status": attachments?.first?[.status] as? Int ?? -1])
    }
    func stream(_ stream: SCStream, didStopWithError error: Error) { print("CAPTURE_ERROR \(error)"); finished = true }
    func recordingOutputDidStartRecording(_ recordingOutput: SCRecordingOutput) { print("RECORDING_STARTED"); fflush(stdout) }
    func recordingOutput(_ recordingOutput: SCRecordingOutput, didFailWithError error: Error) { print("RECORDING_ERROR \(error)"); finished = true }
    func recordingOutputDidFinishRecording(_ recordingOutput: SCRecordingOutput) { print("RECORDING_FINISHED"); finished = true }

    func stop() async {
        try? await stream?.stopCapture()
        for _ in 0..<100 { if finished { break }; try? await Task.sleep(for: .milliseconds(100)) }
        let body: [String: Any] = ["targetFps": 60, "startedAtUnix": startedAt, "finishedAtUnix": Date().timeIntervalSince1970, "frames": frames, "audio": false, "windowOnly": true]
        if let data = try? JSONSerialization.data(withJSONObject: body, options: [.sortedKeys]) { try? data.write(to: output.deletingPathExtension().appendingPathExtension("capture.json")) }
    }
}

#if RECORDER_CLI
@main struct Main {
    static func main() async {
        _ = NSApplication.shared
        guard CommandLine.arguments.count >= 3 else { print("Usage: record-window bundleID output.mp4 [stop-file]"); return }
        guard #available(macOS 15.0, *) else { print("macOS 15+ required"); return }
        let output = URL(fileURLWithPath: CommandLine.arguments[2])
        let stopFile = CommandLine.arguments.count > 3 ? CommandLine.arguments[3] : output.path + ".stop"
        let recorder = Recorder(output: output)
        do {
            try await recorder.start(bundleID: CommandLine.arguments[1])
            while !FileManager.default.fileExists(atPath: stopFile) && !recorder.finished { try await Task.sleep(for: .milliseconds(200)) }
            await recorder.stop()
        } catch { print("CAPTURE_ERROR \(error)"); exit(1) }
    }
}
#endif
