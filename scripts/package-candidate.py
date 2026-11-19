#!/usr/bin/env python3
"""Package an already-built native binary; used by the candidate workflow."""
import argparse, hashlib, json, os, shutil, tarfile, tempfile, zipfile
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--target', required=True)
parser.add_argument('--output', type=Path, default=Path('dist'))
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
version = '0.2.2'
args.output.mkdir(parents=True, exist_ok=True)
windows = 'windows' in args.target
binary_name = 'agent-dlp.exe' if windows else 'agent-dlp'
with tempfile.TemporaryDirectory() as directory:
    package = Path(directory) / 'agent-dlp-lite'
    package.mkdir()
    destination = package / binary_name
    shutil.copyfile(args.binary, destination)
    destination.chmod(0o755)
    digest = hashlib.sha256(destination.read_bytes()).hexdigest()
    for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'README.md', 'README_EN.md']:
        shutil.copyfile(root / name, package / name)
    for name in ['docs', 'examples', 'third-party-licenses']:
        shutil.copytree(root / name, package / name)
    (package / 'scripts').mkdir()
    for name in ['install.sh', 'install.ps1']:
        shutil.copyfile(root / 'scripts' / name, package / 'scripts' / name)
    shutil.copyfile(root / 'examples/policy-pack.yaml', package / 'policy-pack.yaml')
    (package / 'SHA256SUMS').write_text(digest + '  ' + binary_name + '\n', encoding='utf-8')
    (package / 'artifact-manifest.json').write_text(json.dumps({'schema_version':1,'version':version,'target':args.target,'source_commit':os.environ.get('GITHUB_SHA'),'binary':binary_name,'sha256':digest,'signing':'unsigned development candidate'},indent=2)+'\n', encoding='utf-8')
    invocation = '.\\agent-dlp.exe' if windows else './agent-dlp'
    (package / 'START_HERE.txt').write_text(f'''Agent DLP Lite {version} — {args.target}

本包已包含原生可执行文件，无需安装 Rust、Python、Node.js 或其他开发环境。
Extract the archive and open a terminal in this directory. No language runtime is required.

1. 自测 / Self-test:
   {invocation} self-test
2. 检查默认策略 / Validate the supplied policy:
   {invocation} --policy policy-pack.yaml policy validate
3. 预览接入配置 / Preview hook configuration:
   {invocation} --policy policy-pack.yaml integration --agent codex
   {invocation} --policy policy-pack.yaml integration --agent workbuddy

v0.2.2: Codex 使用本地 MCP Hook + 原生交互表单；无需浏览器。
Codex: local stdio MCP hook with a structured confirmation form.
升级后重新 install，并传 --mcp-config 指定 Codex config.toml。
Reinstall with --mcp-config pointing to Codex config.toml; reload and trust updated hooks.
不支持交互、UI 错误或 60 秒超时：告警、记录并继续；明确拒绝/取消则阻断。
Unavailable forms, UI errors or timeout: warn, record, continue. Explicit deny/cancel: block.
已有 deny 规则仍阻断。WorkBuddy 默认仅告警降级，原生表单尚未验证。
Existing deny rules remain blocking. WorkBuddy uses warning fallback by default.
最近 100 条记录区分人工选择和默认继续；查看：{invocation} history list
See docs/confirmation-history.md and docs/installation.md.

SHA256SUMS contains the binary checksum. Read README.md, README_EN.md and docs/installation.md before installing hooks.
候选版未签名/公证，桌面端实际接入范围见 docs/compatibility.md。
Unsigned development candidate; desktop integration acceptance is tracked in docs/compatibility.md.
''', encoding='utf-8')
    stem = 'agent-dlp-lite-' + version + '-' + args.target
    if windows:
        archive = args.output / (stem + '.zip')
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(package.rglob('*')):
                if path.is_file(): bundle.write(path, path.relative_to(package.parent))
        with zipfile.ZipFile(archive) as bundle:
            assert bundle.testzip() is None
            assert 'agent-dlp-lite/' + binary_name in bundle.namelist()
    else:
        archive = args.output / (stem + '.tar.gz')
        with tarfile.open(archive, 'w:gz') as bundle:
            bundle.add(package, arcname=package.name)
        with tarfile.open(archive) as bundle:
            assert bundle.getmember('agent-dlp-lite/' + binary_name).mode & 0o111
    print(json.dumps({'archive':str(archive),'binary_sha256':digest,'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest()}))
