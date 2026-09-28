import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    // macOS has no launch screen: until the first Flutter frame the window shows this colour.
    // It is StrataColors.background (mist #F1F5F7 light, abyss #0F1B26 dark), the colour of
    // the first frame (SplashScreen), so starting shows no black flash.
    let background = NSColor(name: nil) { appearance in
      appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        ? NSColor(srgbRed: 0x0F / 255.0, green: 0x1B / 255.0, blue: 0x26 / 255.0, alpha: 1)
        : NSColor(srgbRed: 0xF1 / 255.0, green: 0xF5 / 255.0, blue: 0xF7 / 255.0, alpha: 1)
    }
    flutterViewController.backgroundColor = background
    self.backgroundColor = background
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
  }
}
