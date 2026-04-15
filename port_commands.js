const fs = require('fs');
const path = require('path');

const srcDir = 'assistant/claude-code';
const geminiDir = '.gemini/commands';
const antigravityDir = '.agent/workflows';
const codexDir = '.codex/skills';

// Ensure directories exist
fs.mkdirSync(geminiDir, { recursive: true });
fs.mkdirSync(antigravityDir, { recursive: true });
fs.mkdirSync(codexDir, { recursive: true });

const files = fs.readdirSync(srcDir).filter(f => f.endsWith('.md'));

for (const file of files) {
    const srcPath = path.join(srcDir, file);
    const content = fs.readFileSync(srcPath, 'utf8');
    
    let frontmatter = '';
    let body = content;
    let name = file.replace('.md', '');
    let description = name;

    if (content.startsWith('---\n')) {
        const endIdx = content.indexOf('---\n', 4);
        if (endIdx !== -1) {
            frontmatter = content.substring(4, endIdx);
            body = content.substring(endIdx + 4).trim();
            
            const descMatch = frontmatter.match(/description:\s*(.+)/);
            if (descMatch) description = descMatch[1].trim();
            
            const nameMatch = frontmatter.match(/name:\s*(.+)/);
            if (nameMatch) name = nameMatch[1].trim();
        }
    }

    // 1. Antigravity (.agent/workflows/*.md)
    const antiPath = path.join(antigravityDir, file);
    const antiContent = `---
name: ${name}
description: ${description}
---

${body}
`;
    fs.writeFileSync(antiPath, antiContent);

    // 2. Gemini CLI (.gemini/commands/*.toml)
    const geminiPath = path.join(geminiDir, name + '.toml');
    const safeBody = body.replace(/\"\"\"/g, "'''");
    const geminiContent = `description = ${JSON.stringify(description)}
prompt = """
${safeBody}

{{args}}
"""
`;
    fs.writeFileSync(geminiPath, geminiContent);

    // 3. Codex (.codex/skills/*/SKILL.md)
    const skillDir = path.join(codexDir, name);
    fs.mkdirSync(skillDir, { recursive: true });
    const codexPath = path.join(skillDir, 'SKILL.md');
    const codexContent = `---
name: ${name}
description: ${description}
license: MIT
compatibility: General
metadata:
  author: assistant
  version: "1.0"
---

${body}
`;
    fs.writeFileSync(codexPath, codexContent);
}

console.log('Porting complete!');