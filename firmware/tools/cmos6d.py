import os; fd=os.open("/dev/port",os.O_RDWR); os.pwrite(fd,bytes([0x6d]),0x70); print("cmos 0x6d =", hex(os.pread(fd,1,0x71)[0]))
