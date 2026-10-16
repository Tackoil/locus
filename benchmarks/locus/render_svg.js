// Rasterize the authored SVG for review; never rewrite its bytes.
const sharp = require(process.env.SVG_BENCH_SHARP || 'sharp');
const [source, destination] = process.argv.slice(2);
if (!source || !destination) throw new Error('Usage: node render_svg.js input.svg preview.png');
sharp(source, { density: 140, limitInputPixels: 100000000 })
  .resize({ width: 1500, height: 1300, fit: 'inside' })
  .flatten({ background: '#ffffff' })
  .png()
  .toFile(destination)
  .catch(error => { process.stderr.write(error.message + '\n'); process.exitCode = 1; });
