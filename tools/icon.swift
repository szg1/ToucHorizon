import AppKit
import CoreGraphics

let size = 512
let out = CommandLine.arguments[1]
let cs = CGColorSpaceCreateDeviceRGB()
let ctx = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
                    space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.interpolationQuality = .none

func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(red: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: a)
}
func rounded(_ r: CGRect, _ radius: CGFloat) -> CGPath {
    CGPath(roundedRect: r, cornerWidth: radius, cornerHeight: radius, transform: nil)
}

// Background: Horizon violet, a soft vertical gradient.
ctx.addPath(rounded(CGRect(x: 0, y: 0, width: size, height: size), 112))
ctx.clip()
let bg = CGGradient(colorsSpace: cs, colors: [rgb(0x3B2A7A), rgb(0x160F2E)] as CFArray, locations: [0, 1])!
ctx.drawLinearGradient(bg, start: CGPoint(x: 0, y: 512), end: CGPoint(x: 0, y: 0), options: [])
// A faint horizon glow behind the bar.
let glow = CGGradient(colorsSpace: cs, colors: [rgb(0x9B7BFF, 0.45), rgb(0x9B7BFF, 0)] as CFArray, locations: [0, 1])!
ctx.drawRadialGradient(glow, startCenter: CGPoint(x: 256, y: 250), startRadius: 0,
                       endCenter: CGPoint(x: 256, y: 250), endRadius: 260, options: [])

// The Touch Bar strip.
let bar = CGRect(x: 36, y: 186, width: 440, height: 140)
ctx.setShadow(offset: CGSize(width: 0, height: -10), blur: 30, color: rgb(0x000000, 0.6))
ctx.addPath(rounded(bar, 30)); ctx.setFillColor(rgb(0x050507)); ctx.fillPath()
ctx.setShadow(offset: .zero, blur: 0, color: nil)
ctx.addPath(rounded(bar.insetBy(dx: 1.5, dy: 1.5), 29)); ctx.setStrokeColor(rgb(0xFFFFFF, 0.14))
ctx.setLineWidth(3); ctx.strokePath()

// Pixel art (original), 12x12, '.' transparent.
let palette: [Character: UInt32] = [
    "w": 0xF2F2F2, "l": 0xB9E8FF, "c": 0x4FC3F7, "d": 0x1E6E9E, "k": 0x2B2B33,
    "b": 0x7A4E2D, "B": 0x5A3820, "g": 0x5BBF3A, "G": 0x3E8F27, "y": 0xFFD34D,
    "o": 0xE89A2C, "r": 0xE5533D, "R": 0xA8321F, "s": 0x8C8C99, "S": 0x5E5E6B,
]
let sword = [
    "..........ww",
    ".........wlw",
    "........wlcw",
    ".......wlcw.",
    "......wlcw..",
    ".....wlcw...",
    "..y.wlcw....",
    "..yywcw.....",
    "...yyw......",
    "..bByyy.....",
    ".bB..y......",
    "bB..........",
]
let block = [
    "gggggggggggg",
    "gGggggGggggg",
    "ggggGgggggGg",
    "bGbbgbbGbgbb",
    "bbbbbbbbbbbb",
    "bBbbbbBbbbbb",
    "bbbbBbbbbBbb",
    "bbbbbbbbbbbb",
    "bBbbbbbbBbbb",
    "bbbbbBbbbbbb",
    "bbBbbbbbbbBb",
    "bbbbbbbbbbbb",
]
let gem = [
    "............",
    "...wwwwww...",
    "..wllccccd..",
    ".wllccccccd.",
    "wlccccccccdd",
    ".dccccccccd.",
    "..dccccccd..",
    "...dccccd...",
    "....dccd....",
    ".....dd.....",
    "............",
    "............",
]
let apple = [
    "......B.....",
    ".....B.g....",
    "....rBrgg...",
    "..rrrrrrrr..",
    ".rrwrrrrrrR.",
    ".rwrrrrrrrR.",
    ".rrrrrrrrrR.",
    ".rrrrrrrrRR.",
    "..rrrrrrRR..",
    "..RrrrrrRR..",
    "...RR..RR...",
    "............",
]
func pixels(_ art: [String], in r: CGRect) {
    let px = r.width / 12
    for (row, line) in art.enumerated() {
        for (col, ch) in line.enumerated() {
            guard let hex = palette[ch] else { continue }
            ctx.setFillColor(rgb(hex))
            ctx.fill(CGRect(x: r.minX + CGFloat(col) * px, y: r.maxY - CGFloat(row + 1) * px,
                            width: px, height: px).insetBy(dx: -0.25, dy: -0.25))
        }
    }
}
func count(_ text: String, at p: CGPoint) {
    let font = NSFont.systemFont(ofSize: 30, weight: .heavy)
    for (dx, dy, color) in [(2.5, -2.5, NSColor.black), (0.0, 0.0, NSColor.white)] {
        let s = NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: color])
        let line = CTLineCreateWithAttributedString(s)
        let w = CTLineGetTypographicBounds(line, nil, nil, nil)
        ctx.textPosition = CGPoint(x: p.x - CGFloat(w) + dx, y: p.y + dy)
        CTLineDraw(line, ctx)
    }
}

// Four cells: the selected hotbar slot (outlined), two more hotbar slots, then the inventory.
let cell: CGFloat = 92, gap: CGFloat = 12
let x0 = bar.minX + 22, y0 = bar.midY - cell / 2
let items: [([String], String?, CGFloat)] = [(sword, nil, 0.30), (block, "64", 0.16), (apple, "12", 0.16), (gem, "3", 0.07)]
for (i, (art, n, tile)) in items.enumerated() {
    let r = CGRect(x: x0 + CGFloat(i) * (cell + gap), y: y0, width: cell, height: cell)
    ctx.addPath(rounded(r, 14)); ctx.setFillColor(rgb(0xFFFFFF, tile)); ctx.fillPath()
    if i == 0 {
        ctx.addPath(rounded(r.insetBy(dx: 2.5, dy: 2.5), 12)); ctx.setStrokeColor(rgb(0xFFFFFF))
        ctx.setLineWidth(5); ctx.strokePath()
    }
    pixels(art, in: r.insetBy(dx: 16, dy: 16))
    if let n { count(n, at: CGPoint(x: r.maxX - 6, y: r.minY + 6)) }
}
// A divider between the hotbar and the inventory, as on the bar.
ctx.setFillColor(rgb(0xFFFFFF, 0.25))
ctx.fill(CGRect(x: x0 + 3 * (cell + gap) - gap / 2 - 1.5, y: y0 + 14, width: 3, height: cell - 28))

let image = ctx.makeImage()!
let rep = NSBitmapImageRep(cgImage: image)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
