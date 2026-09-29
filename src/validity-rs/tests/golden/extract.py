# Extract golden vectors from python-validity (run as root with python3-validity stopped).
import json, random, sys
from binascii import hexlify
from struct import pack
h = lambda b: hexlify(bytes(b)).decode()
from validitysensor import init, tls as tlsmod, timeslot as prg
from validitysensor.tls import tls, prf, hs_key
from validitysensor.usb import usb
from validitysensor.sensor import sensor, RomInfo, identify_sensor, get_factory_bits, CaptureMode, bitpack
from validitysensor.flash import read_tls_flash, get_flash_info, get_fw_info
from validitysensor.db import db
from validitysensor import blobs_9a
out = {}
out['blobs'] = {k: h(getattr(blobs_9a, k)) for k in ('init_hardcoded','init_hardcoded_clean_slate','reset_blob','db_write_enable')}
out['prf'] = [{'secret': h(s), 'seed': h(sd), 'len': n, 'out': h(prf(s, sd, n))} for s, sd, n in
              [(b'k'*16, b'seed', 0x20), (bytes(range(32)), b'master secret'+bytes(64), 0x30), (b'\x01'*48, b'key expansion'+bytes(64), 0x120)]]
out['hs_key'] = '%064x' % hs_key()
out['psk_enc'] = h(tls.psk_encryption_key); out['psk_val'] = h(tls.psk_validation_key)
random.seed(1)
bp = [[random.randrange(0,256) for _ in range(random.randrange(4,120))] for _ in range(6)] + [[0x7e,0x7f,0x80]*20]
out['bitpack'] = [{'in': h(b), 'u': r[0], 'm': r[1], 'out': h(r[2])} for b in bp for r in [bitpack(bytes(b))]]
init.open()
out['tls_flash'] = h(read_tls_flash())
ri = RomInfo.get(); out['rom_info'] = vars(ri)
di = identify_sensor(); out['dev_info'] = {'major': di.major, 'type': di.type, 'version': di.version, 'name': di.name}
ti = sensor.type_info
out['type_info'] = {'sensor_type': ti.sensor_type, 'bytes_per_line': ti.bytes_per_line, 'repeat_multiplier': ti.repeat_multiplier,
                    'lines_per_calibration_data': ti.lines_per_calibration_data, 'line_width': ti.line_width, 'calibration_blob': h(ti.calibration_blob)}
out['lines_per_frame'] = sensor.lines_per_frame
out['hardcoded_prog'] = h(sensor.hardcoded_prog)
out['factory_bits'] = {str(k): h(v) for k, v in get_factory_bits(0x0e00).items()}
out['calib_data'] = h(sensor.calib_data)
out['cmd02'] = {m.name: h(sensor.build_cmd_02(m)) for m in CaptureMode}
fi = get_flash_info(); out['flash_info'] = {'ic': fi.ic.name, 'blocks': fi.blocks, 'blocksize': fi.blocksize, 'parts': [vars(p) for p in fi.partitions]}
fw = get_fw_info(2); out['fw_info'] = {'major': fw.major, 'minor': fw.minor, 'buildtime': fw.buildtime, 'modules': len(fw.modules)}
# raw DB replies for parser tests
stg = db.get_user_storage(name='StgWindsor')
out['db_storage_rsp'] = h(tls.cmd(pack('<BHH', 0x4b, 0, 11) + b'StgWindsor\0'))
out['db_users'] = []
for u in stg.users:
    raw = tls.cmd(pack('<BHHH', 0x4a, u['dbid'], 0, 0)); usr = db.get_user(u['dbid'])
    out['db_users'].append({'raw': h(raw), 'identity': repr(usr.identity), 'fingers': usr.fingers})
out['db_info'] = vars(db.db_info())
# synthetic calibration pipeline vectors (average + process_calibration_results)
random.seed(2)
frame = sensor.lines_per_frame * ti.bytes_per_line
raw = bytes(random.randrange(0,256) for _ in range(frame * (sensor.calibration_frames) + ti.bytes_per_line))
saved = sensor.calib_data
sensor.calib_data = b''
avg = sensor.average(raw); sensor.process_calibration_results(avg); c1 = sensor.calib_data
sensor.process_calibration_results(avg); c2 = sensor.calib_data
sensor.calib_data = saved
out['calib_vec'] = {'raw': h(raw), 'avg': h(avg), 'c1': h(c1), 'c2': h(c2), 'frames': sensor.calibration_frames, 'key_line': sensor.key_calibration_line}
json.dump(out, open(sys.argv[1], 'w'), indent=1, default=str)
print('ok', len(json.dumps(out, default=str)))
