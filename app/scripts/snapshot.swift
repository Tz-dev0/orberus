// Renders the app's interface to a PNG, for the README.
//
//   npm run dev                      (in app/, in another terminal)
//   swift app/scripts/snapshot.swift light docs/images/app-light.png
//   swift app/scripts/snapshot.swift dark  docs/images/app-dark.png
//
// It loads the browser preview (sample data, made-up accounts) in a web view
// that is never shown, so it needs no screen recording permission. The real
// window is drawn by macOS as a frosted panel; here a plain panel stands in.
// A third argument is JavaScript to run first, for example to open a section.
import AppKit
import WebKit

let arguments = CommandLine.arguments
guard arguments.count >= 3 else {
    print("usage: snapshot.swift light|dark <out.png> [javascript]")
    exit(2)
}
let dark = arguments[1] == "dark"
let output = URL(fileURLWithPath: arguments[2])
let script = arguments.count > 3 ? arguments[3] : ""
let width: CGFloat = 320 // the real window, see tauri.conf.json

final class Renderer: NSObject, WKNavigationDelegate {
    let view: WKWebView

    override init() {
        view = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: 900))
        super.init()
        view.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        // Nothing behind the page: outside the rounded panel the PNG is clear.
        view.setValue(false, forKey: "drawsBackground")
        view.navigationDelegate = self
        view.load(URLRequest(url: URL(string: "http://localhost:1420/")!))
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // Round the stand-in panel like the real window, then let the page
        // finish its opening animation before measuring it.
        let prepare = """
        const style = document.createElement("style");
        style.textContent = "html{background:transparent!important}"
          + "body{border-radius:12px;box-shadow:inset 0 0 0 1px var(--line)}"
          + "*{animation-duration:0s!important;animation-delay:0s!important;transition:none!important}";
        document.head.appendChild(style);
        """
        view.evaluateJavaScript(prepare) { _, _ in
            self.after(1.5) {
                self.view.evaluateJavaScript(script.isEmpty ? "0" : script) { _, _ in
                    self.after(1.5) { self.measure() }
                }
            }
        }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        fail("\(error.localizedDescription)")
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        fail("\(error.localizedDescription) (is `npm run dev` running in app/?)")
    }

    func after(_ seconds: Double, _ work: @escaping () -> Void) {
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds, execute: work)
    }

    func measure() {
        view.evaluateJavaScript("Math.ceil(document.querySelector('.page').getBoundingClientRect().height)") { value, _ in
            let height = (value as? NSNumber).map { CGFloat(truncating: $0) } ?? 600
            self.view.setFrameSize(NSSize(width: width, height: height))
            self.after(0.5) { self.snapshot(height) }
        }
    }

    func snapshot(_ height: CGFloat) {
        let configuration = WKSnapshotConfiguration()
        configuration.rect = NSRect(x: 0, y: 0, width: width, height: height)
        // In points: on a Retina Mac the image comes out at twice that in pixels.
        configuration.snapshotWidth = NSNumber(value: Double(width))
        view.takeSnapshot(with: configuration) { image, error in
            guard let image, let tiff = image.tiffRepresentation,
                  let bitmap = NSBitmapImageRep(data: tiff),
                  let png = bitmap.representation(using: .png, properties: [:])
            else { fail(error?.localizedDescription ?? "no image") }
            do { try png.write(to: output) } catch { fail("\(error)") }
            print("wrote \(output.path) (\(bitmap.pixelsWide)x\(bitmap.pixelsHigh))")
            exit(0)
        }
    }
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write("snapshot: \(message)\n".data(using: .utf8)!)
    exit(1)
}

let application = NSApplication.shared
application.setActivationPolicy(.prohibited) // no Dock icon, no window
let renderer = Renderer()
application.run()
