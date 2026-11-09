// Renders screenshots/banner.png (1920x1080) from screenshots/touchbar.png:
//   swift tools/banner.swift screenshots/touchbar.png screenshots/banner.png
import AppKit
import CoreGraphics

let args = CommandLine.arguments
let strip = NSImage(contentsOfFile: args[1])!.cgImage(forProposedRect: nil, context: nil, hints: nil)!
let W = 1920, H = 1080
let cs = CGColorSpaceCreateDeviceRGB()
let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: 0, space: cs,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!

func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(red: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: a)
}
func rounded(_ r: CGRect, _ radius: CGFloat) -> CGPath {
    CGPath(roundedRect: r, cornerWidth: radius, cornerHeight: radius, transform: nil)
}
/// y measured from the top, as in a layout.
func top(_ y: CGFloat, _ h: CGFloat) -> CGFloat { CGFloat(H) - y - h }

func text(_ s: String, size: CGFloat, weight: NSFont.Weight, alpha: CGFloat, centerX: CGFloat, baselineFromTop: CGFloat) {
    let font = NSFont.systemFont(ofSize: size, weight: weight)
    let attr = NSAttributedString(string: s, attributes: [.font: font,
        .foregroundColor: NSColor(white: 1, alpha: alpha)])
    let line = CTLineCreateWithAttributedString(attr)
    let w = CTLineGetTypographicBounds(line, nil, nil, nil)
    ctx.textPosition = CGPoint(x: centerX - CGFloat(w) / 2, y: CGFloat(H) - baselineFromTop)
    CTLineDraw(line, ctx)
}

/// Draws part `src` of the strip (pixels, from its left) into `dst`, inside a Touch Bar bezel.
func bar(_ src: CGRect, _ dst: CGRect, smooth: Bool) {
    let pad: CGFloat = dst.height * 0.16
    let bezel = dst.insetBy(dx: -pad, dy: -pad)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -12), blur: 36, color: rgb(0x000000, 0.65))
    ctx.addPath(rounded(bezel, bezel.height * 0.24)); ctx.setFillColor(rgb(0x050507)); ctx.fillPath()
    ctx.restoreGState()
    ctx.addPath(rounded(bezel.insetBy(dx: 1.5, dy: 1.5), bezel.height * 0.24 - 1.5))
    ctx.setStrokeColor(rgb(0xFFFFFF, 0.14)); ctx.setLineWidth(3); ctx.strokePath()
    let part = strip.cropping(to: src)!
    ctx.interpolationQuality = smooth ? .high : .none
    ctx.draw(part, in: dst)
}

// Background: the icon's violet, with a glow behind the bar.
let bg = CGGradient(colorsSpace: cs, colors: [rgb(0x3B2A7A), rgb(0x140E2A)] as CFArray, locations: [0, 1])!
ctx.drawLinearGradient(bg, start: CGPoint(x: 0, y: H), end: CGPoint(x: 0, y: 0), options: [])
let glow = CGGradient(colorsSpace: cs, colors: [rgb(0x9B7BFF, 0.35), rgb(0x9B7BFF, 0)] as CFArray, locations: [0, 1])!
ctx.drawRadialGradient(glow, startCenter: CGPoint(x: 960, y: top(470, 0)), startRadius: 0,
                       endCenter: CGPoint(x: 960, y: top(470, 0)), endRadius: 900, options: [])

text("ToucHorizon", size: 104, weight: .heavy, alpha: 1, centerX: 960, baselineFromTop: 200)
text("Your hotbar and inventory on the Touch Bar", size: 42, weight: .medium, alpha: 0.72,
     centerX: 960, baselineFromTop: 272)

// The whole strip, as wide as fits.
let sw = CGFloat(strip.width), sh = CGFloat(strip.height)
let scale = 1760 / sw
let full = CGRect(x: (1920 - sw * scale) / 2, y: top(410, sh * scale), width: sw * scale, height: sh * scale)
bar(CGRect(x: 0, y: 0, width: sw, height: sh), full, smooth: true)

// Close-ups at 2x: the first five hotbar slots, and the inventory cells.
let hot = CGRect(x: 0, y: 0, width: 380, height: sh)
let inv = CGRect(x: 908, y: 0, width: 304, height: sh)
let zoom: CGFloat = 2
let gap: CGFloat = 150
let total = (hot.width + inv.width) * zoom + gap
let left = (1920 - total) / 2
let zy = top(640, sh * zoom)
let hotDst = CGRect(x: left, y: zy, width: hot.width * zoom, height: sh * zoom)
let invDst = CGRect(x: hotDst.maxX + gap, y: zy, width: inv.width * zoom, height: sh * zoom)
bar(hot, hotDst, smooth: false)
bar(inv, invDst, smooth: false)
text("Tap a hotbar item to select it", size: 36, weight: .semibold, alpha: 0.95,
     centerX: hotDst.midX, baselineFromTop: 870)
text("Tap an inventory item to swap it", size: 36, weight: .semibold, alpha: 0.95,
     centerX: invDst.midX, baselineFromTop: 870)
text("into the selected slot", size: 36, weight: .semibold, alpha: 0.95,
     centerX: invDst.midX, baselineFromTop: 916)
text("Swipe to scroll all 36 slots  ·  ⇄ swaps with the off hand", size: 32, weight: .regular,
     alpha: 0.6, centerX: 960, baselineFromTop: 1010)

let image = ctx.makeImage()!
try! NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])!
    .write(to: URL(fileURLWithPath: args[2]))
