with open('src/main.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Find lines to delete: 238-266 (second duplicate block)
# Line 238 (0-indexed 237) starts with {"cat":...
new_lines = []
skip = False
for i, line in enumerate(lines, 1):
    if i == 238:
        skip = True
        continue
    if i == 267:
        skip = False
        continue
    if not skip:
        new_lines.append(line)

with open('src/main.rs', 'w', encoding='utf-8') as f:
    f.writelines(new_lines)

print("Deleted lines 238-266")
