# MicroBrute SysEx

## General info
```
inquiry1  01 59 00 37
reply     01 59 01 36 02 01 00 00 00 00 00 00 00   01 = major version?
inquiry2  01 5a 00 39
reply     01 5a 01 38 08 04 00 00 00 00 00 00 00   04 = minor version?
```

## Update seq
```
0x01 MSGID(u8) SEQ(0x23, 0x3a) SEQ_ID(u8) SEQ_OFFSET(u8) SEQ_LEN(u8, max 0x20) SEQ_NOTES([u8; 32] 0 padded, start@ C0=0x30, C#0 0x31... rest=0x7f)
```

## Query seq
Request
```
0x01 MSGID(u8) 0x03,0x3b(SEQ) SEQ_IDX(u8 0 - 7) 0x00 SEQ_OFFSET(u8) SEQ_LEN(0x20)
```

Reply
```
0x01 MSGID(u8) SEQ(0x23) SEQ_ID(u8) SEQ_OFFSET(u8) SEQ_LEN(u8, max 0x20) SEQ_NOTES([u8; 32] 0 padded, start@ C0=0x30, C#0 0x31... rest=0x7f)
```

## beatstep 00 20 6b 7f
update payload
```
          field   control  value
42 02 00  01      70       09
```

control
- `0x70..0x7f` Pads

Pad fields
- 0x01 Off=0x0/Mmc/Switched/Note/ProgramChange
 

controller
fields 