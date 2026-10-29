"""Sample the complete native process without foreground interaction.

Starts the normal UI path hidden, seeds 100 items through HTTP MCP, and
samples the entire process (not only Rust heap). No user data/config is changed.
"""
import ctypes, json, os, socket, sqlite3, subprocess, sys, time, urllib.request
from pathlib import Path
from ctypes import wintypes as W

ROOT=Path(__file__).resolve().parents[1]
EXE=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else ROOT/'target/release/glass-todo.exe'
DATA=ROOT/'.scratch'/('qa-resource-'+str(time.time_ns()))
DATA.mkdir(parents=True)
dock_edge=os.environ.get('GLASS_TODO_TEST_DOCK_EDGE')
if dock_edge:
    assert dock_edge in ('left','right','top','bottom')
    with sqlite3.connect(DATA/'todos.sqlite3') as connection:
        connection.execute('CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL)')
        connection.execute('INSERT INTO settings VALUES(?,?)',('window',json.dumps({'x':0,'y':0,'width':316,'height':366,'dock':{'edge':dock_edge,'position':0.5}})))
with socket.socket() as s:
    s.bind(('127.0.0.1',0));port=s.getsockname()[1]
startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0
proc=subprocess.Popen([str(EXE),'--data-dir',str(DATA),'--port',str(port),'--no-autostart','--exit-after','20'],startupinfo=startup,creationflags=subprocess.CREATE_NO_WINDOW)
headers={'Content-Type':'application/json','Accept':'application/json, text/event-stream','MCP-Protocol-Version':'2025-03-26'}
seq=0
def call(method,params=None,notify=False):
    global seq
    seq+=1;data={'jsonrpc':'2.0','method':method}
    if not notify:data['id']=seq
    if params is not None:data['params']=params
    with urllib.request.urlopen(urllib.request.Request(f'http://127.0.0.1:{port}/mcp',data=json.dumps(data).encode(),headers=headers),timeout=8) as r:
        if r.getheader('Mcp-Session-Id'):headers['Mcp-Session-Id']=r.getheader('Mcp-Session-Id')
        if r.status==202:return None
        if 'text/event-stream' in r.headers.get('Content-Type',''):
            for line in r:
                if line.startswith(b'data:') and line[5:].strip():return json.loads(line[5:])
        raw=r.read();return json.loads(raw) if raw else None
def tool(name,args):
    result=call('tools/call',{'name':name,'arguments':args})['result']
    assert not result.get('isError'),result
    return json.loads(result['content'][0]['text'])

class Counters(ctypes.Structure):
    _fields_=[('cb',W.DWORD),('PageFaultCount',W.DWORD)]+[(name,ctypes.c_size_t) for name in ['PeakWorkingSetSize','WorkingSetSize','QuotaPeakPagedPoolUsage','QuotaPagedPoolUsage','QuotaPeakNonPagedPoolUsage','QuotaNonPagedPoolUsage','PagefileUsage','PeakPagefileUsage','PrivateUsage','PrivateWorkingSetSize','SharedCommitUsage']]
kernel=ctypes.WinDLL('kernel32',use_last_error=True)
kernel.OpenProcess.argtypes=[W.DWORD,W.BOOL,W.DWORD];kernel.OpenProcess.restype=W.HANDLE
kernel.CloseHandle.argtypes=[W.HANDLE]
kernel.GetProcessTimes.argtypes=[W.HANDLE,ctypes.POINTER(W.FILETIME),ctypes.POINTER(W.FILETIME),ctypes.POINTER(W.FILETIME),ctypes.POINTER(W.FILETIME)]
psapi=ctypes.WinDLL('psapi',use_last_error=True);psapi.GetProcessMemoryInfo.argtypes=[W.HANDLE,ctypes.POINTER(Counters),W.DWORD]
def snapshot(handle):
    info=Counters();info.cb=ctypes.sizeof(info)
    if not psapi.GetProcessMemoryInfo(handle,ctypes.byref(info),info.cb):raise ctypes.WinError(ctypes.get_last_error())
    times=[W.FILETIME() for _ in range(4)]
    if not kernel.GetProcessTimes(handle,*[ctypes.byref(t) for t in times]):raise ctypes.WinError(ctypes.get_last_error())
    cpu=sum((t.dwHighDateTime<<32)|t.dwLowDateTime for t in times[2:])/10_000_000
    return info,cpu

try:
    for _ in range(100):
        try:
            token=(DATA/'mcp-token.txt').read_text().strip()
            with socket.create_connection(('127.0.0.1',port),timeout=.1):break
        except OSError:time.sleep(.1)
    headers['Authorization']='Bearer '+token
    call('initialize',{'protocolVersion':'2025-03-26','capabilities':{},'clientInfo':{'name':'resource-check','version':'1'}})
    call('notifications/initialized',notify=True)
    for i in range(100):tool('create_todo',{'content':f'待办 {i+1}：检查项目记录与接口状态'})
    first=tool('list_todos',{})[0]
    tool('update_todo',{'id':first['id'],'status':'in_progress','expected_version':first['version']})
    handle=kernel.OpenProcess(0x0410,False,proc.pid)
    if not handle:raise ctypes.WinError(ctypes.get_last_error())
    try:
        settle=float(os.environ.get('GLASS_TODO_SETTLE_SECONDS','2'))
        time.sleep(settle);a,cpu_a=snapshot(handle);start=time.perf_counter();time.sleep(5);b,cpu_b=snapshot(handle);elapsed=time.perf_counter()-start
        result={'scenario':'normal native window path, hidden to avoid foreground interaction; 100 todos; 1 active MCP session','private_working_set_bytes':b.PrivateWorkingSetSize,'working_set_bytes':b.WorkingSetSize,'private_commit_bytes':b.PrivateUsage,'peak_working_set_bytes':b.PeakWorkingSetSize,'idle_cpu_percent_one_core':round((cpu_b-cpu_a)/elapsed*100,3),'sample_seconds':round(elapsed,3),'settle_seconds':settle,'exe_bytes':EXE.stat().st_size}
        result['dock_edge']=dock_edge
        result_path=ROOT/'.scratch'/('resource-results-dock-'+dock_edge+'.json' if dock_edge else 'resource-results.json')
        result_path.write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result,indent=2))
    finally:kernel.CloseHandle(handle)
    proc.wait(timeout=25)
    assert proc.returncode==0,f'Graceful timed exit failed: {proc.returncode}'
    assert 'visible=false' in (DATA/'app.log').read_text(encoding='utf-8'), 'Resource checks must keep the window hidden'
    if dock_edge:
        with sqlite3.connect(DATA/'todos.sqlite3') as connection:
            window=json.loads(connection.execute("SELECT value FROM settings WHERE key='window'").fetchone()[0])
        assert window['dock']['edge']==dock_edge and (window['width'],window['height'])==(316,366)
    print('PASS: graceful timed exit after active MCP session')
finally:
    if proc.poll() is None:proc.kill();proc.wait(timeout=10)
