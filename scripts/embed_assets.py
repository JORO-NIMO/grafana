import os

def to_c_byte_array(data, name):
    lines = [f"static const unsigned char {name}[] = {{"]
    for i in range(0, len(data), 16):
        chunk = data[i:i+16]
        hex_bytes = ", ".join(f"0x{b:02x}" for b in chunk)
        lines.append(f"  {hex_bytes},")
    lines.append("  0x00")
    lines.append("};")
    lines.append(f"static const unsigned int {name}_len = {len(data)};\n")
    return "\n".join(lines)

with open('web/index.html', 'rb') as f:
    html_data = f.read()
with open('web/style.css', 'rb') as f:
    css_data = f.read()
with open('web/gafana_web.js', 'rb') as f:
    js_data = f.read()

output_path = 'src/gafana_web_assets.h'
with open(output_path, 'w', encoding='utf-8') as out:
    out.write("/* Auto-generated embedded web assets for Gafana single-executable */\n")
    out.write("#ifndef GAFANA_WEB_ASSETS_H\n#define GAFANA_WEB_ASSETS_H\n\n")
    out.write(to_c_byte_array(html_data, "s_embedded_html"))
    out.write(to_c_byte_array(css_data, "s_embedded_css"))
    out.write(to_c_byte_array(js_data, "s_embedded_js"))
    out.write("#endif /* GAFANA_WEB_ASSETS_H */\n")

print(f"Generated {output_path} with {len(html_data)}B HTML, {len(css_data)}B CSS, {len(js_data)}B JS.")
