import json
def cs(b):
    a=0
    for c in b.encode(): a^=c
    return a
def s(b): return f"${b}*{cs(b):02X}\r\n"
o=""
for t in range(700):
    h,m,sec=12+t//3600,t//60%60,t%60
    tod=f"{h:02}{m:02}{sec:02}.00"
    o+=s(f"GPRMC,{tod},A,6900.0000,N,01600.0000,E,0.0,0.0,150925,,,A")
    o+=s(f"GPGGA,{tod},6900.0000,N,01600.0000,E,1,08,1.0,10.0,M,0.0,M,,")
    hit=300<=t<=360
    for st in range(2):
        b=f"GPGSV,2,{st+1},08"
        for k in range(4):
            sv=st*4+k+1
            snr=45.0-(sv%3)-(20 if hit else 0)
            b+=f",{sv:02},{30+sv*5:02},{sv*40:03},{snr:02.0f}"
        o+=s(b)
open("jt/log.nmea","w",newline="").write(o)
json.dump({"test_types":[{"type":"Jamming","type_id":1,"test_groups":[{"group_id":2,"group_name":"CW","tests":[{"test_id":5,"name":"CW jammer"}]}]}]},open("jt/catalogue.json","w"))
def sl(a,b): return {"start_time":f"2025-09-15T{a}","end_time":f"2025-09-15T{b}","test_id":"1.2.5","power_w":0,"status":""}
json.dump({"locations":[{"location_id":1,"location_name":"Test Area 1","tests":[sl("12:05:00","12:06:00"),sl("12:08:20","12:09:20")]}]},open("jt/plan.json","w"))
