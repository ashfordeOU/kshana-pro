# Builds a NovAtel OEM7 ASCII log for the review of the self-report strip: BESTPOSA + RANGEA + RXSTATUSA once per second, with the layout of
# W56's own test lines (machines/novatel.rs). The values are invented; the receiver's jammer flag is set 120..180 s and C/N0 falls 90..150 s.
import sys
POLY=0xEDB88320
def crc(b):
    c=0
    for x in b:
        t=(c^x)&0xFF
        for _ in range(8): t=(t>>1)^POLY if t&1 else t>>1
        c=((c>>8)&0x00FFFFFF)^t
    return c
assert crc(b'BESTPOSA,COM1,0,50.0,FINESTEERING,2300,100.000,02000000,0000,1;SOL_COMPUTED,SINGLE,59.91150000000,10.75790000000,52.0000,40.0000,WGS84,1.0000,1.0000,2.0000,"",0.000,0.000,14,12,12,12,00,00,00,00')==0xa82c1454
def line(s): return f"#{s}*{crc(s.encode()):08x}\r\n"
out=[]
gps=0x00000004; gal=(3<<16)|(2<<21)|4
for t in range(0,300):
    sec=100+t
    drop = 90<=t<=150
    cn=(44.5-14 if drop else 44.5); cn2=(38.0-14 if drop else 38.0)
    jam = 120<=t<=180
    lat=59.9115+t*1e-6
    out.append(line(f'BESTPOSA,COM1,0,50.0,FINESTEERING,2300,{sec}.000,02000000,0000,1;SOL_COMPUTED,SINGLE,{lat:.11f},10.75790000000,52.0000,40.0000,WGS84,1.0000,1.0000,2.0000,"",0.000,0.000,14,12,12,12,00,00,00,00'))
    out.append(line(f'RANGEA,COM1,0,50.0,FINESTEERING,2300,{sec}.000,02000000,5103,1;2,5,0,2.4e7,0.2,-1.2e8,0.01,100.0,{cn},500.0,{gps:08x},11,0,2.1e7,0.2,-1.2e8,0.01,100.0,{cn2},500.0,{gal:08x}'))
    out.append(line(f'RXSTATUSA,COM1,0,50.0,FINESTEERING,2300,{sec}.000,02000000,5103,1;00000000,2,{"00008000" if jam else "00000000"},0,0,0,{"00000001" if jam else "00000000"},0,0,0'))
open(sys.argv[1],'w',newline='').write(''.join(out))
