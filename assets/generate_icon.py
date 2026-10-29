"""Generate the small application icon using only the Python standard library."""
import math, struct, zlib
from pathlib import Path

def segment_distance(x,y,a,b):
    dx,dy=b[0]-a[0],b[1]-a[1]
    t=max(0,min(1,((x-a[0])*dx+(y-a[1])*dy)/(dx*dx+dy*dy)))
    return math.hypot(x-a[0]-t*dx,y-a[1]-t*dy)

def chunk(tag,data):
    return struct.pack('>I',len(data))+tag+data+struct.pack('>I',zlib.crc32(tag+data)&0xffffffff)

def png(n):
    rows=[]
    for y in range(n):
        row=bytearray([0])
        for x in range(n):
            u,v=(x+.5)/n,(y+.5)/n
            qx,qy=max(abs(u-.5)-.28,0),max(abs(v-.5)-.28,0)
            inside=math.hypot(qx,qy)<.18
            a=255 if inside else 0
            rgb=(int(52+35*v),int(118+35*v),int(174+35*v))
            if min(segment_distance(u,v,(.26,.5),(.43,.66)),segment_distance(u,v,(.43,.66),(.75,.32)))<.047 and inside:
                rgb=(246,253,255)
            row.extend((*rgb,a))
        rows.append(bytes(row))
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',n,n,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(rows),9))+chunk(b'IEND',b'')

sizes=[16,32,48,64,256]
images=[png(n) for n in sizes]
offset=6+16*len(images)
entries=[]
for n,data in zip(sizes,images):
    entries.append(struct.pack('<BBBBHHII',n%256,n%256,0,0,1,32,len(data),offset))
    offset+=len(data)
Path(__file__).with_name('app.ico').write_bytes(struct.pack('<HHH',0,1,len(images))+b''.join(entries)+b''.join(images))
