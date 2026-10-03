#!/usr/bin/env python3
"""Package the glibc Tauri build with a static PRoot compatibility launcher.
This is NOT a native musl build or a security sandbox. Shell uses the bundled
userspace; project/home mounts refer to host files. Build on the same glibc
system as the Tauri executable. Requires proot.static, mksquashfs, static
AppImage type2 runtime. Arguments: AppDir, proot.static, runtime, output.
"""
import os,re,sys,shutil,subprocess
from pathlib import Path
source,proot,runtime,out=map(Path,sys.argv[1:]);target=out.parent/'Velocity-Alpine.AppDir'
if target.exists():shutil.rmtree(target)
target.mkdir(parents=True);root=target/'rootfs';shutil.copytree(source,root,symlinks=True)
(root/'AppRun').unlink(missing_ok=True)
for d in ['lib64','lib','etc','etc/ssl/certs','proc','sys','dev','tmp','run','home','mnt','media','opt','var/tmp']: (root/d).mkdir(parents=True,exist_ok=True)
# Keep all loader-interpreted paths absolute within the PRoot userspace.
def copyfile(src,dst=None):
 src=Path(src);dst=root/(str(dst or src).lstrip('/'));dst.parent.mkdir(parents=True,exist_ok=True)
 if src.exists():shutil.copy2(src.resolve(),dst);return dst
for name in ['coreutils','sh','bash','env','ls','cat','pwd','mkdir','rm','cp','mv','touch','head','tail','wc','find','grep','sed','sort','git','curl','timeout','uname','id','which']:
 src=shutil.which(name)
 if src:copyfile(src)
# sh is generally a symlink to bash in /usr/bin on Amazon Linux.
(root/'bin').mkdir(exist_ok=True);copyfile(shutil.which('bash'),'/bin/sh')
copyfile('/lib64/ld-linux-x86-64.so.2');copyfile('/lib64/ld-linux-x86-64.so.2','/lib/ld-linux-x86-64.so.2')
# Resolve the dependencies of ALL ELF files, including WebKit subprocesses/modules.
queue=[]
for p in root.rglob('*'):
 if p.is_file() and not p.is_symlink():
  try:
   with p.open('rb') as f:
    if f.read(4)==b'\x7fELF':queue.append(p)
  except OSError:pass
seen=set()
while queue:
 p=queue.pop()
 if str(p) in seen:continue
 seen.add(str(p));r=subprocess.run(['ldd',str(p)],capture_output=True,text=True)
 for path in re.findall(r'(?:=>\s*)?(/[^\s()]+)',r.stdout):
  src=Path(path)
  if str(src).startswith(str(root)):continue
  if src.exists():
   dst=root/path.lstrip('/')
   if not dst.exists():copyfile(src);queue.append(dst)
# Config/resources used via dlopen or filesystem lookups rather than ELF linkage.
for path in ['/etc/fonts','/usr/share/fontconfig','/usr/share/fonts/dejavu','/usr/share/glib-2.0/schemas','/usr/share/X11/xkb','/usr/lib64/gio/modules','/usr/lib64/gdk-pixbuf-2.0']:
 src=Path(path)
 if src.exists():shutil.copytree(src,root/path.lstrip('/'),dirs_exist_ok=True,symlinks=False)
for path in ['/lib64/libnss_dns.so.2','/lib64/libnss_files.so.2','/lib64/libresolv.so.2']:
 copyfile(path)
copyfile('/etc/pki/tls/certs/ca-bundle.crt','/etc/ssl/certs/ca-certificates.crt')
shutil.copy2(proot,target/'proot.static');os.chmod(target/'proot.static',0o755)
for p in source.glob('*.desktop'):shutil.copy2(p,target/p.name)
for p in source.glob('*.png'):shutil.copy2(p,target/p.name)
app='''#!/bin/sh
set -eu
HERE="${APPDIR:-$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)}"
# No system glibc/gcompat required. Static PRoot redirects loader and library paths.
# It does not isolate the app from host files. Never run this as root.
# Amazon Linux WebKitGTK embeds helper paths relative to /usr; start there so
# ././libexec and ././lib64 resolve inside the bundled userspace, not host CWD.
# The original CWD remains bind-mounted at its absolute path for project access.
exec "$HERE/proot.static" -R "$HERE/rootfs" -b /run -b /home -b /mnt -b /media -b "$PWD" -w /usr /usr/bin/env \\
 LD_LIBRARY_PATH=/usr/lib:/usr/lib64:/lib64:/lib \\
 WEBKIT_EXEC_PATH=/usr/libexec/webkit2gtk-4.1 \\
 WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 \\
 GSETTINGS_SCHEMA_DIR=/usr/share/glib-2.0/schemas \\
 /usr/bin/velocity-harness "$@"
'''
(target/'AppRun').write_text(app);os.chmod(target/'AppRun',0o755)
(target/'ALPINE-COMPATIBILITY.txt').write_text('Bundled glibc userspace via static PRoot. NOT a native musl build. NOT a sandbox. WebKit sandbox is disabled because PRoot uses ptrace. Do not run as root. Shell tools run in bundled userspace, not Alpine apk environment. Host project files are mounted. Graphics session and ptrace support required. Use --appimage-extract-and-run if FUSE unavailable.\n')
image=out.with_suffix('.squashfs');subprocess.run(['mksquashfs',str(target),str(image),'-noappend','-comp','zstd','-processors','4','-quiet'],check=True)
with out.open('wb') as f:
 with runtime.open('rb') as x:shutil.copyfileobj(x,f)
 with image.open('rb') as x:shutil.copyfileobj(x,f)
os.chmod(out,0o755);image.unlink();print(str(out),out.stat().st_size)
