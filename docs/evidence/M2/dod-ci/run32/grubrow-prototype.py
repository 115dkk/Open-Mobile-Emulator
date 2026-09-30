# SPDX-License-Identifier: GPL-2.0-or-later
# Prototype of the GRUB selection-bar reader ported to ci/dod/native.ps1 (-Highlight); run against the
# captured frames of runs 27 to 32 and the dev PC frames to check the geometry rules are scale-free.
import zlib, struct, sys
def read_png(path):
    data=open(path,'rb').read(); assert data[:8]==b'\x89PNG\r\n\x1a\n'
    pos=8; idat=b''; w=h=None; ct=None
    while pos<len(data):
        ln,=struct.unpack('>I',data[pos:pos+4]); typ=data[pos+4:pos+8]; body=data[pos+8:pos+8+ln]; pos+=12+ln
        if typ==b'IHDR': w,h,bd,ct,_,_,il=struct.unpack('>IIBBBBB',body); assert bd==8 and il==0
        elif typ==b'IDAT': idat+=body
        elif typ==b'IEND': break
    raw=zlib.decompress(idat); bpp={2:3,6:4,0:1}[ct]; stride=w*bpp; out=[]; prev=bytearray(stride); i=0
    for y in range(h):
        f=raw[i]; line=bytearray(raw[i+1:i+1+stride]); i+=1+stride
        for x in range(stride):
            a=line[x-bpp] if x>=bpp else 0; b=prev[x]; c=prev[x-bpp] if x>=bpp else 0
            if f==1: line[x]=(line[x]+a)&255
            elif f==2: line[x]=(line[x]+b)&255
            elif f==3: line[x]=(line[x]+((a+b)>>1))&255
            elif f==4:
                p=a+b-c; pa=abs(p-a); pb=abs(p-b); pc=abs(p-c)
                pr=a if pa<=pb and pa<=pc else (b if pb<=pc else c); line[x]=(line[x]+pr)&255
        out.append(bytes(line)); prev=line
    return w,h,bpp,out
def analyze(path):
    w,h,bpp,rows=read_png(path)
    def px(x,y): o=x*bpp; r=rows[y]; return r[o],r[o+1],r[o+2]
    blue=lambda r,g,b: b>=120 and b>r+50 and b>g+20
    white=lambda r,g,b: r>200 and g>200 and b>200
    # bar rows: fraction of blue across x in [0.25w,0.75w]
    xs=list(range(int(0.25*w),int(0.75*w),4)); bar=[]
    for y in range(0,h,2):
        n=sum(1 for x in xs if blue(*px(x,y)))
        bar.append((y, n/len(xs)))
    barrows=[y for y,f in bar if f>0.5]
    clusters=[]
    for y in barrows:
        if clusters and y-clusters[-1][-1]<=4: clusters[-1].append(y)
        else: clusters.append([y])
    big=max(clusters,key=len) if clusters else None
    ybar=(big[0]+big[-1])/2 if big else None
    # entry rows: icon column band
    band=list(range(int(0.19*w),int(0.24*w),2)); pres=[]
    for y in range(0,h,1):
        n=sum(1 for x in band if blue(*px(x,y)) or white(*px(x,y)))
        pres.append(n>=2)
    ent=[]; cur=None
    for y,p in enumerate(pres):
        if p:
            if cur and y-cur[-1]<=3: cur.append(y)
            else:
                if cur: ent.append(cur)
                cur=[y]
    if cur: ent.append(cur)
    ent=[c for c in ent if 6<=(c[-1]-c[0])<=60]
    centers=[(c[0]+c[-1])/2 for c in ent]
    idx=min(range(len(centers)), key=lambda i: abs(centers[i]-ybar)) if centers and ybar is not None else None
    return dict(w=w,h=h,bar=ybar,barHeight=(big[-1]-big[0]) if big else None,entries=[round(c) for c in centers],row=idx)
for p in sys.argv[1:]:
    print(p.split('/')[-3], p.split('/')[-1], analyze(p))
