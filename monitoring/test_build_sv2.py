# Installer integration tests with fake dependencies and native ELF test binaries.
# No package installation, real service operation, or production path write occurs.
from pathlib import Path
import os,subprocess,tempfile,shutil,json
repo=Path(__file__).resolve().parent
revision='bef6dd2efa2ed803d77fb49fbc064666685cb742'
with tempfile.TemporaryDirectory(prefix='bdl-sv2-installer-test-') as tmp:
 root=Path(tmp);fakebin=root/'fakebin';fakebin.mkdir()
 c=root/'fake.c';c.write_text(r'''
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifndef ENABLED
#define ENABLED 0
#endif
int main(int argc,char **argv){
 if(argc>1&&!strcmp(argv[1],"--help")){puts("-T | --testconfig");return 0;}
 int enabled=ENABLED;
 const char *target=getenv("BDL_TEST_INSTALLED");
 if(enabled&&target&&getenv("BDL_TEST_FAIL_INSTALLED")&&!strcmp(argv[0],target))enabled=0;
 printf("sv2_compiled = %d\nrpcpassword = SECRET\n",enabled);return 0;
}
''')
 binaries=[]
 for enabled in (0,1):
  binary=root/f'fake-{enabled}';subprocess.run(['gcc',f'-DENABLED={enabled}',str(c),'-o',str(binary)],check=True,capture_output=True);binaries.append(binary)
 def executable(name,source):
  path=fakebin/name;path.write_text('#!/usr/bin/python3\n'+source);path.chmod(0o755)
 executable('id','print("bdl")\n')
 executable('sudo', '''import os,sys,json
with open(os.environ['BDL_TEST_COMMAND_LOG'],'a') as log: log.write(json.dumps(sys.argv[1:])+'\\n')
if sys.argv[1] == '-v': raise SystemExit(0)
if sys.argv[1] == 'apt-get': print('APT simulated'); raise SystemExit(0)
os.execvp(sys.argv[1],sys.argv[1:])
''')
 executable('git', '''import sys,pathlib
args=sys.argv[1:]; directory=pathlib.Path(args[1]); action=args[2]
if action=='init': (directory/'.git').mkdir(exist_ok=True)
if action=='checkout':
 (directory/'src').mkdir(exist_ok=True)
 for name in ('autogen.sh','configure'):
  script=directory/name; script.write_text('#!/bin/sh\\nprintf "#define HAVE_SV2 1\\\\n" > config.h\\n'); script.chmod(0o755)
if action=='rev-parse': print('''+repr(revision)+''')
''')
 executable('make', '''import os,shutil,pathlib
assert '--jobs=1' in __import__('sys').argv or '-j1' in __import__('sys').argv
shutil.copy2(os.environ['BDL_TEST_CANDIDATE'],pathlib.Path.cwd()/'src/ckpool')
''')
 for mode in ('success','candidate-rejected','rollback'):
  case=root/mode;case.mkdir();home=case/'pool-home';(home/'mining/ckpool').mkdir(parents=True);target=case/'bin/ckpool';target.parent.mkdir();shutil.copy2(binaries[0],target)
  config=home/'mining/ckpool/local.conf';config.write_text('{"btcd":[{"pass":"PRIVATE_TEST"}],"serverurl":["0.0.0.0:3333"]}')
  scripts=case/'scripts';scripts.mkdir();shutil.copy2(repo/'inspect-sv2.py',scripts/'inspect-sv2.py')
  script=scripts/'build-sv2.sh';script.write_text((repo/'build-sv2.sh').read_text().replace('pool_home=/home/bdl','pool_home='+str(home)).replace('installed=/usr/local/bin/ckpool','installed='+str(target)))
  assert 'pool_home=/home/bdl' not in script.read_text() and 'installed=/usr/local/bin/ckpool' not in script.read_text()
  env=os.environ.copy();env['PATH']=str(fakebin)+':'+env['PATH'];env['BDL_TEST_CANDIDATE']=str(binaries[0 if mode=='candidate-rejected' else 1]);env['BDL_TEST_COMMAND_LOG']=str(case/'commands.jsonl');env['BDL_TEST_INSTALLED']=str(target)
  if mode=='rollback':env['BDL_TEST_FAIL_INSTALLED']='1'
  result=subprocess.run(['bash',str(script)],env=env,capture_output=True,text=True,timeout=20)
  assert 'SECRET' not in result.stdout+result.stderr,(mode,result.stdout,result.stderr)
  assert config.read_text()=='{"btcd":[{"pass":"PRIVATE_TEST"}],"serverurl":["0.0.0.0:3333"]}'
  backups=list(target.parent.glob('ckpool.before-sv2-*'))
  if mode=='success':
   assert result.returncode==0,(result.stdout,result.stderr)
   assert target.read_bytes()==binaries[1].read_bytes();assert len(backups)==1 and backups[0].read_bytes()==binaries[0].read_bytes()
   manifest=json.loads((home/'.local/state/bdl-pool-monitor/sv2-build.json').read_text())
   assert manifest['revision']==revision and manifest['backupBinary']==str(backups[0])
   assert Path(manifest['buildDirectory']).is_dir()
   rerun=subprocess.run(['bash',str(script)],env=env,capture_output=True,text=True,timeout=10)
   assert rerun.returncode==0 and 'déjà le support SV2' in rerun.stdout
   assert len(list(target.parent.glob('ckpool.before-sv2-*')))==1
  else:
   assert result.returncode!=0,(mode,result.stdout,result.stderr)
   assert target.read_bytes()==binaries[0].read_bytes()
   assert bool(backups)==(mode=='rollback')
  commands=[json.loads(line) for line in (case/'commands.jsonl').read_text().splitlines()]
  assert not any('systemctl' in command for command in commands)
  print(mode+': guard, backup/atomic replacement, private-output filtering and unchanged configuration verified')
