import AppKit
import CoreGraphics
import Foundation
import ApplicationServices

// Only inspect/activate the exact new process passed by the smoke test.
let args = CommandLine.arguments
if args.count < 3 { fatalError("usage: window-probe PID output.png [keycode]") }
let pid = pid_t(args[1])!
NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateIgnoringOtherApps])
Thread.sleep(forTimeInterval: 0.15)
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
let matches = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == Int(pid) && ($0[kCGWindowLayer as String] as? Int ?? 0) <= 3 }
guard let window = matches.max(by: { a, b in
    let x = a[kCGWindowBounds as String] as? [String:Double] ?? [:]
    let y = b[kCGWindowBounds as String] as? [String:Double] ?? [:]
    return (x["Width"] ?? 0)*(x["Height"] ?? 0) < (y["Width"] ?? 0)*(y["Height"] ?? 0)
}) else { fatalError("No visible application window for PID \(pid)") }
let bounds = window[kCGWindowBounds as String] as! [String:Double]
let id = window[kCGWindowNumber as String] as! Int
let position = CGPoint(x: (bounds["X"] ?? 0)+60, y: (bounds["Y"] ?? 0)+100)
CGWarpMouseCursorPosition(position)
CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: position, mouseButton: .left)?.post(tap: .cghidEventTap)
if args.count > 3, let code = UInt16(args[3]) {
    CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: true)?.postToPid(pid)
    CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: false)?.postToPid(pid)
}
Thread.sleep(forTimeInterval: 0.25)
let shot = Process()
shot.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
shot.arguments = ["-x", "-o", "-l", String(id), args[2]]
try shot.run(); shot.waitUntilExit()
if shot.terminationStatus != 0 { fatalError("Window capture failed: \(shot.terminationStatus)") }
var result: [String:Any] = ["pid":Int(pid), "window_id":id, "bounds":bounds, "title":window[kCGWindowName as String] ?? "", "layer":window[kCGWindowLayer as String] ?? 0, "capture":args[2], "screen_capture_allowed":CGPreflightScreenCaptureAccess(), "accessibility_trusted":AXIsProcessTrusted()]
if let bitmap = NSBitmapImageRep(data: try Data(contentsOf: URL(fileURLWithPath: args[2]))) {
    let points: [(Double, Double)] = [(0.4,0.5), (0.6,0.35), (0.75,0.5), (0.8,0.6)]
    result["image_samples_rgb"] = points.map { point -> [Int] in
        let color = bitmap.colorAt(x: Int(Double(bitmap.pixelsWide)*point.0), y: Int(Double(bitmap.pixelsHigh)*point.1))!.usingColorSpace(.sRGB)!
        return [color.redComponent, color.greenComponent, color.blueComponent].map { Int(($0*255).rounded()) }
    }
}
let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
print(String(data:data,encoding:.utf8)!)
