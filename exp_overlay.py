import csv
import ctypes
import datetime
import json
import os
import queue
import re
import threading
import time
import tkinter as tk
import traceback
from ctypes import wintypes

try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)
except Exception:
    try:
        ctypes.windll.user32.SetProcessDPIAware()
    except Exception:
        pass

user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
imm32 = ctypes.windll.imm32
user32.GetForegroundWindow.restype = wintypes.HWND
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.BringWindowToTop.argtypes = [wintypes.HWND]
user32.GetAncestor.argtypes = [wintypes.HWND, wintypes.UINT]
user32.GetAncestor.restype = wintypes.HWND
user32.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
user32.GetWindowLongW.restype = wintypes.LONG
user32.SetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int, wintypes.LONG]
user32.SetWindowLongW.restype = wintypes.LONG
user32.SetWindowPos.argtypes = [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int,
                                ctypes.c_int, ctypes.c_int, wintypes.UINT]
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowThreadProcessId.restype = wintypes.DWORD
user32.AttachThreadInput.argtypes = [wintypes.DWORD, wintypes.DWORD, wintypes.BOOL]
user32.MessageBoxW.argtypes = [wintypes.HWND, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.UINT]
kernel32.GetCurrentThreadId.restype = wintypes.DWORD
imm32.ImmAssociateContextEx.argtypes = [wintypes.HWND, ctypes.c_void_p, wintypes.DWORD]
HWND_TOPMOST = wintypes.HWND(-1)
import sys
HERE = os.path.dirname(os.path.abspath(sys.executable if getattr(sys, 'frozen', False) else __file__))


def _writable(d):
    try:
        p = os.path.join(d, '.write_test')
        open(p, 'w').close()
        os.remove(p)
        return True
    except OSError:
        return False


if not _writable(HERE):
    HERE = os.path.join(os.environ.get('APPDATA') or os.path.expanduser('~'), 'ArtaleExpOverlay')
    os.makedirs(HERE, exist_ok=True)
CFG_PATH = os.path.join(HERE, 'config.json')
LOG_PATH = os.path.join(HERE, '練功紀錄.csv')
ERR_PATH = os.path.join(HERE, 'error.log')
STATE_PATH = os.path.join(HERE, 'session.json')
RESUME_HOURS = 2


DETAIL_PATH = os.path.join(HERE, '明細紀錄.log')


def log_detail(text):
    try:
        if os.path.exists(DETAIL_PATH) and os.path.getsize(DETAIL_PATH) > 2_000_000:
            os.replace(DETAIL_PATH, DETAIL_PATH + '.old')
        with open(DETAIL_PATH, 'a', encoding='utf-8') as f:
            f.write(f'{datetime.datetime.now():%m-%d %H:%M:%S}  {text}\n')
    except Exception:
        pass


def log_error(text):
    try:
        if os.path.exists(ERR_PATH) and os.path.getsize(ERR_PATH) > 1_000_000:
            os.replace(ERR_PATH, ERR_PATH + '.old')
        with open(ERR_PATH, 'a', encoding='utf-8') as f:
            f.write(f'--- {datetime.datetime.now():%Y-%m-%d %H:%M:%S}\n{text}\n')
    except Exception:
        pass


def new_mss():
    import mss
    return (getattr(mss, 'MSS', None) or mss.mss)()

DEFAULT = dict(
    hp_name='馴鹿奶', hp_price=5600, hp_heal=5000,
    mp_name='紅豆刨冰', mp_price=3800, mp_heal=2000,
    stack_max=3000, ocr_sec=3, alpha=0.88, x=40, y=40, unit=600,
    compact=False,
    exp_region=None, hp_region=None, mp_region=None, inv_region=None, meso_region=None,
)

C = dict(bg='#141922', panel='#1f2735', line='#2e384b', ink='#e6eaf2', muted='#8b95a8',
         exp='#f0c050', hp='#ff6b61', mp='#6ea2ff', good='#4fcf8f', warn='#ffa25c')
FONT = 'Microsoft JhengHei UI'
MONO = 'Consolas'
ROUND_FONTS = ('jf open 粉圓 2.1', 'jf open 粉圓 2.0', 'jf open 粉圓', '源泉圓體月 M', '源泉圓體 M',
               'Zen Maru Gothic Medium', 'Zen Maru Gothic')


def pick_fonts():
    global FONT, MONO
    import tkinter.font as tkfont
    fams = set(tkfont.families())
    for name in ROUND_FONTS:
        if name in fams:
            FONT = MONO = name
            return


def tpl_path(k):
    return os.path.join(HERE, f'tpl_{k}.png')


def load_cfg():
    cfg = dict(DEFAULT)
    try:
        with open(CFG_PATH, encoding='utf-8') as f:
            cfg.update(json.load(f))
    except Exception:
        pass
    for k in ('remind_min', 'sound'):
        cfg.pop(k, None)
    return cfg


def short(n):
    if n is None or n != n or n in (float('inf'), float('-inf')):
        return '–'
    a = abs(n)
    if a >= 1e8:
        return f'{n / 1e8:.2f}億'
    if a >= 1e6:
        return f'{n / 1e4:.0f}萬'
    if a >= 1e4:
        return f'{n / 1e4:.1f}萬'
    return f'{n:,.0f}'


def dur(sec):
    if sec is None or sec != sec or sec < 0 or sec == float('inf'):
        return '–'
    m = int(sec // 60)
    if m < 60:
        return f'{m}分'
    h = m // 60
    if h < 48:
        return f'{h}時{m % 60:02d}分'
    return f'{h // 24}天{h % 24}時'


def hms(sec):
    s = int(max(0, sec))
    return f'{s // 3600:02d}:{s // 60 % 60:02d}:{s % 60:02d}'


def parse_exp(text):
    t = re.sub(r'\s+', '|', text.replace(',', '').replace('，', '').replace('．', '.'))
    if '%' in t or '[' in t:
        m = next((x for x in re.finditer(r'(\d{1,3}(?:\.\d{1,3})?)\|?%', t) if float(x.group(1)) <= 100), None) \
            or re.search(r'\[\|?(\d{1,3}\.\d{1,3})', t)
        if not m:
            return None
        pct = float(m.group(1))
        seg = re.split(r'[\]/]', t[:m.start()])[-1]
        nums = re.findall(r'\d+', seg)
        if not nums:
            return None
        return int(max(nums, key=len)), pct
    for m in re.finditer(r'(\d+)/(\d+)', t):
        if re.search(r'(HP|MP)\W*$', t[:m.start()], re.I):
            continue
        e, n = int(m.group(1)), int(m.group(2))
        if 0 < n and e <= n:
            return e, e * 100 / n
    return None


def parse_meso(text):
    t = re.sub(r'\s+', '', (text or '').replace('，', ',').replace('．', '.'))
    t = t.strip(':;.,\'’`|')
    if re.fullmatch(r'\d+', t):
        return int(t)
    if re.fullmatch(r'\d{1,3}(?:[,.:;\'’`|]\d{3})+', t):
        return int(re.sub(r'\D', '', t))
    return None


def fix_digits(t):
    t = (t or '').strip()
    return t.translate(str.maketrans({'一': '1', '丨': '1', '|': '1', 'l': '1', 'I': '1', 'O': '0', 'o': '0'}))


def bag_count(t):
    t = (t or '').strip('.,:;。，、· ').replace(',', '')
    return t if re.fullmatch(r'[1-9]\d*', t) else ''


def parse_count(text):
    t = re.sub(r'F\d{1,2}(?!\d)', ' ', text.replace(',', '').replace('，', ''))
    nums = re.findall(r'\d+', t)
    return int(nums[-1]) if nums else None


def flatten(res):
    boxes = [it for it in (res or []) if isinstance(it, (list, tuple)) and len(it) >= 2
             and isinstance(it[1], str) and not isinstance(it[0], str)]
    if boxes:
        boxes.sort(key=lambda it: min(p[0] for p in it[0]))
        return '|'.join(it[1] for it in boxes)
    out = []
    for it in res or []:
        if isinstance(it, str):
            out.append(it)
        elif isinstance(it, (list, tuple)):
            out += [x for x in it if isinstance(x, str)]
    return ' '.join(out)


class OcrWorker(threading.Thread):
    def __init__(self, app):
        super().__init__(daemon=True)
        self.app = app
        self.wake = threading.Event()
        self.tpl_cache = {}
        self.seen = {}
        self.errs = {}
        self.grab_fails = 0
        self.bag_id = 0
        self.was_open = False

    def run(self):
        try:
            from rapidocr_onnxruntime import RapidOCR
            try:
                self.engine = RapidOCR(det_limit_type='max', det_limit_side_len=960,
                                       intra_op_num_threads=2, inter_op_num_threads=1,
                                       det_intra_op_num_threads=2, rec_intra_op_num_threads=2,
                                       cls_intra_op_num_threads=2)
            except Exception:
                self.engine = RapidOCR()
            self.sct = new_mss()
        except Exception as e:
            log_error(traceback.format_exc())
            self.app.q.put(('ocr_err', f'OCR 無法載入：{e}'))
            return
        try:
            kernel32.GetCurrentThread.restype = wintypes.HANDLE
            kernel32.SetThreadPriority.argtypes = [wintypes.HANDLE, ctypes.c_int]
            kernel32.SetThreadPriority(kernel32.GetCurrentThread(), -1)
        except Exception:
            log_error(traceback.format_exc())
        self.app.q.put(('ocr_ready', None))
        fast = 0
        while True:
            bag = False
            try:
                bag = self.tick()
                self.ok('loop')
            except Exception as e:
                self.fail('loop', e)
                self.app.q.put(('ocr_err', f'自動讀取出錯：{e}'))
            fast = fast + 1 if bag else 0
            delay = 0.4 if 0 < fast <= 12 else max(1.0, float(self.app.cfg.get('ocr_sec', 3)))
            self.wake.wait(delay)
            self.wake.clear()

    def fail(self, key, e):
        msg = f'{type(e).__name__}: {e}'
        if self.errs.get(key) != msg:
            self.errs[key] = msg
            log_error(f'[{key}] ' + traceback.format_exc())
        if key in ('exp', 'hp', 'mp', 'meso', 'inv'):
            self.grab_fails += 1
            if self.grab_fails >= 3:
                self.grab_fails = 0
                try:
                    self.sct.close()
                except Exception:
                    pass
                try:
                    self.sct = new_mss()
                except Exception:
                    pass
        return msg

    def ok(self, key):
        if self.errs.pop(key, None) is not None:
            log_error(f'[{key}] 恢復正常')
        if key != 'loop':
            self.grab_fails = 0

    def tick(self):
        out = {}
        for k, parser in (('exp', parse_exp), ('hp', parse_count), ('mp', parse_count)):
            r = self.app.cfg.get(k + '_region')
            if not r:
                continue
            try:
                img = self.grab(r)
                if k == 'exp':
                    text = self.unchanged('exp', img)
                    if text is None:
                        text = self.read(img, parser, wide=True)
                        self.remember('exp', text)
                    out[k] = text
                else:
                    out[k] = self.read_count(img, key=k)
                self.ok(k)
            except Exception as e:
                out[k] = ''
                out[k + '_err'] = self.fail(k, e)
        inv = self.app.cfg.get('inv_region')
        inv_open = False
        try:
            tpls = self.load_tpls()
            if inv and tpls:
                out['inv'] = self.read_inventory(inv, tpls)
                inv_open = any(out['inv'].values())
            self.ok('inv')
        except Exception as e:
            self.fail('inv', e)
        if inv_open and not self.was_open:
            self.bag_id += 1
        self.was_open = inv_open
        out['bag_id'] = self.bag_id
        meso = self.app.cfg.get('meso_region')
        if meso and inv_open:
            try:
                out['meso'] = self.read_count(self.grab(meso), key='meso', digits=True)
                self.ok('meso')
            except Exception as e:
                self.fail('meso', e)
        else:
            self.seen.pop('meso', None)
        if len(out) > 1:
            self.app.q.put(('ocr', out))
        return inv_open

    def load_tpls(self):
        import cv2
        import numpy as np
        from PIL import Image
        tpls = {}
        for k in ('hp', 'mp'):
            p = tpl_path(k)
            if not os.path.exists(p):
                self.tpl_cache.pop(k, None)
                continue
            mt = os.path.getmtime(p)
            if k not in self.tpl_cache or self.tpl_cache[k][0] != mt:
                cell = np.array(Image.open(p).convert('RGB'))[:, :, ::-1].copy()
                ch, cw = cell.shape[:2]
                top = cell[:max(4, int(ch * 0.55))].copy()
                sig = max(1.0, cw / 55)
                blur = cv2.GaussianBlur(top, (0, 0), sig)
                hsv = cv2.cvtColor(blur, cv2.COLOR_BGR2HSV)
                lab = cv2.cvtColor(blur, cv2.COLOR_BGR2LAB).astype(np.float32)
                colored = (hsv[:, :, 1] > 90) & (hsv[:, :, 2] > 60)
                self.tpl_cache[k] = (mt, dict(top=top, cw=cw, ch=ch, bright=float(top.mean()), sig=sig, blur=blur,
                                              hue=hsv[:, :, 0].astype(np.int16), colored=colored,
                                              a=lab[:, :, 1] - 128, b=lab[:, :, 2] - 128))
            tpls[k] = self.tpl_cache[k][1]
        return tpls

    @staticmethod
    def same_item(t, patch):
        import cv2
        import numpy as np
        hsv = cv2.cvtColor(patch, cv2.COLOR_BGR2HSV)
        mask = t['colored']
        if mask.sum() >= 30:
            dh = np.abs(hsv[:, :, 0].astype(np.int16) - t['hue'])[mask]
            return float(np.minimum(dh, 180 - dh).mean()) <= 5
        lab = cv2.cvtColor(patch, cv2.COLOR_BGR2LAB).astype(np.float32)
        a, b = lab[:, :, 1] - 128, lab[:, :, 2] - 128
        m = (np.hypot(t['a'], t['b']) > 12) | (np.hypot(a, b) > 12)
        return not m.any() or float(np.hypot(a - t['a'], b - t['b'])[m].mean()) <= 10

    def read_inventory(self, r, tpls):
        import cv2
        import numpy as np
        from PIL import Image
        img = np.array(self.grab(r))[:, :, ::-1].copy()
        H, W = img.shape[:2]
        res = {}
        for k, t in tpls.items():
            cw, ch, bright = t['cw'], t['ch'], t['bright']
            th, tw = t['top'].shape[:2]
            if H < th or W < tw:
                res[k] = []
                continue
            img_blur = cv2.GaussianBlur(img, (0, 0), t['sig'])
            m = cv2.matchTemplate(img_blur, t['blur'], cv2.TM_CCOEFF_NORMED)
            hits, dim = [], False
            while len(hits) < 80:
                _, mx, _, (x, y) = cv2.minMaxLoc(m)
                if mx < 0.6:
                    break
                patch_bright = float(img[y:y + th, x:x + tw].mean())
                if not self.same_item(t, img_blur[y:y + th, x:x + tw]):
                    pass
                elif abs(patch_bright - bright) <= 20:
                    if mx >= 0.8:
                        hits.append((x, y))
                elif mx >= 0.8 or patch_bright < bright - 20:
                    dim = True
                m[max(0, y - th // 2):y + th // 2 + 1, max(0, x - tw // 2):x + tw // 2 + 1] = -1
            if dim:
                res[k] = [None]
                continue
            stacks = []
            for x, y in sorted(hits, key=lambda p: (p[1], p[0])):
                y0, y1 = y + int(ch * 0.45), min(H, y + int(ch * 1.05))
                crop = img[y0:y1, max(0, x - int(cw * 0.2)):min(W, x + int(cw * 1.08))]
                sy0, sy1 = y + int(ch * 0.5), min(H, y + int(ch * 0.95))
                strip = img[sy0:sy1, max(0, x):min(W, x + cw)]
                if crop.size == 0:
                    stacks.append(None)
                    continue
                text = self.read_count(Image.fromarray(crop[:, :, ::-1].copy()), key=f'inv_{k}_{x}_{y}', strict=True,
                                       fallback=Image.fromarray(strip[:, :, ::-1].copy()) if strip.size else None)
                stacks.append(parse_count(text))
            res[k] = stacks
        return res

    def grab(self, r):
        from PIL import Image
        x, y, w, h = r
        s = self.sct.grab({'left': x, 'top': y, 'width': w, 'height': h})
        return Image.frombytes('RGB', s.size, s.bgra, 'raw', 'BGRX')

    @staticmethod
    def prep(img):
        import numpy as np
        from PIL import Image
        w, h = img.size
        scale = 3 if h < 30 else 2 if h < 60 else 1
        if scale > 1:
            img = img.resize((w * scale, h * scale), Image.LANCZOS)
        pad = Image.new('RGB', (img.width + 24, img.height + 24), img.getpixel((0, 0)))
        pad.paste(img, (12, 12))
        return np.array(pad)[:, :, ::-1].copy()

    def rec_only(self, img):
        try:
            res, _ = self.engine(self.prep(img), use_det=False, use_cls=False)
        except TypeError:
            res, _ = self.engine(self.prep(img))
        return flatten(res)

    def unchanged(self, key, img):
        import numpy as np
        g = np.asarray(img.convert('L'), dtype=np.int16)
        old = self.seen.get(key)
        if old is not None and old[1] is not None and old[0].shape == g.shape \
                and np.count_nonzero(np.abs(old[0] - g) > 40) <= 2:
            return old[1]
        self.seen[key] = (g, None)
        return None

    def remember(self, key, text):
        if key in self.seen:
            self.seen[key] = (self.seen[key][0], text)

    def read_count(self, img, key=None, strict=False, fallback=None, digits=False):
        if key:
            cached = self.unchanged(key, img)
            if cached is not None:
                return cached
        arr = self.prep(img)
        try:
            res, _ = self.engine(arr)
        except Exception:
            res = None
        boxes = []
        for it in res or []:
            if not (isinstance(it, (list, tuple)) and len(it) >= 2 and isinstance(it[1], str)):
                continue
            t = fix_digits(it[1]) if strict else it[1]
            if digits and not re.fullmatch(r"[\d,.:;'’`\s]+", t):
                continue
            if strict or re.search(r'\d', t):
                ys = [p[1] for p in it[0]]
                xs = [p[0] for p in it[0]]
                conf = it[2] if len(it) > 2 and isinstance(it[2], (int, float)) else 1.0
                boxes.append(((min(ys) + max(ys)) / 2, min(xs), min(ys), max(ys), t, conf))
        if strict:
            boxes = [b for b in boxes if b[4].strip()]
        if boxes:
            cy, _, y0, y1 = max(boxes)[:4]
            tol = max(6, (y1 - y0) * 0.5)
            row = sorted((b for b in boxes if abs(b[0] - cy) <= tol), key=lambda b: b[1])
            text = ''.join(b[4] for b in row)
            if strict:
                text = bag_count(text)
                if not text and fallback is not None and max(b[5] for b in row) < 0.8:
                    text = bag_count(fix_digits(self.rec_only(fallback)))
        elif strict:
            text = ''
            if fallback is not None:
                text = bag_count(fix_digits(self.rec_only(fallback)))
        elif digits:
            text = ''
        else:
            text = self.rec_only(img)
        if key and (not (strict or digits) or re.search(r'\d', text)):
            self.remember(key, text)
        return text

    def read(self, img, parser, wide):
        arr = self.prep(img)
        modes = [{}, {'use_det': False, 'use_cls': False}]
        if not wide:
            modes.reverse()
        last = ''
        for kw in modes:
            try:
                res, _ = self.engine(arr, **kw)
            except TypeError:
                res, _ = self.engine(arr)
            text = flatten(res)
            if text and parser(text) is not None:
                return text
            last = text or last
        return last


def hotkey_loop(q):
    MOD_CONTROL, MOD_NOREPEAT = 0x2, 0x4000
    keys = {1: (0x79, 'pause', 'Ctrl+F10'), 2: (0x7A, 'record', 'Ctrl+F11'), 3: (0x7B, 'toggle', 'Ctrl+F12')}
    failed = [name for i, (vk, _, name) in keys.items()
              if not user32.RegisterHotKey(None, i, MOD_CONTROL | MOD_NOREPEAT, vk)]
    if failed:
        q.put(('hotkey_fail', failed))
    msg = wintypes.MSG()
    while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) != 0:
        if msg.message == 0x0312 and msg.wParam in keys:
            q.put(('hotkey', keys[msg.wParam][1]))


class App:
    PAIR_ROWS = [('exp', 'EXP', 'exp'), ('hp', '', 'hp'), ('mp', '', 'mp'), ('cost', '藥水花費', 'hp'),
                 ('income', '撿錢', 'exp'), ('profit', '淨賺', 'good')]
    ONE_ROWS = [('gain', '本次獲得', 'ink'), ('earn', '本次收益', 'good'), ('lv', '升級', 'mp'),
                ('left_hp', '', 'hp'), ('left_mp', '', 'mp')]

    def __init__(self):
        self.cfg = load_cfg()
        self.q = queue.Queue()
        self.root = tk.Tk()
        pick_fonts()
        self.root.report_callback_exception = self.on_error
        self.set_win = self.rec_win = self.prev_fg = None
        try:
            self.quit_evt = kernel32.CreateEventW(None, False, False, QUIT_EVENT)
        except Exception:
            self.quit_evt = None
        self.raw = {'exp': '', 'hp': '', 'mp': ''}
        self.alert = None
        self.hotkey_msg = ''
        self.ocr_msg = '讀取引擎載入中…'
        self.ocr_seen = 0.0
        self.reset_armed = 0.0
        self.running = self.paused = False
        self.exp_last = None
        self.need = None
        self.pend_first = None
        P = lambda v: {'hp': v, 'mp': v}
        self.qs, self.qs_pend, self.qs_t = P(None), P(None), P(0.0)
        self.qs_seen = P(0.0)
        self.qs_credit = P(0)
        self.fill_from = time.time()
        self.qs_all, self.qs_add = P(0), P(0)
        self.total = P(None)
        self.total_act = P(0.0)
        self.inv_pend, self.inv_slots = P(None), P(None)
        self.last_rise = P(None)
        self.cap_since = P(None)
        self.inv_msg = ''
        self.meso, self.meso_pend = None, None
        self.spent_all = 0
        self.meso_spent = 0
        self.meso_t = 0.0
        self.qs_sus = P(0)
        self.cur_bag_id = 0
        self.meso_bag = None
        self.exp_ok_t = 0.0
        self.new_session()
        self.build()
        self.load_state()
        self.ocr = OcrWorker(self)
        self.ocr.start()
        threading.Thread(target=hotkey_loop, args=(self.q,), daemon=True).start()
        self.root.after(200, self.poll)
        self.root.after(500, self.refresh)
        self.root.after(1500, self.keep_top)
        self.root.after(60000, self.autosave)

    SESSION_KEYS = ('gain', 'hist', 'levelups', 'deaths', 'qs_used', 'bag_used', 'meso_anchor', 'series',
                    'qs_sus_cnt', 'last_drop', 'inc_done', 'seg_pending')
    KEEP_KEYS = ('need', 'qs_all', 'qs_add', 'total', 'inv_slots', 'last_rise',
                 'spent_all', 'meso', 'meso_spent', 'meso_t', 'qs_sus')

    def save_state(self):
        try:
            data = {k: getattr(self, k) for k in self.SESSION_KEYS + self.KEEP_KEYS}
            data.update(saved_at=time.time(), acc=self.active(), running=self.running,
                        started_at=self.started_at.isoformat() if self.started_at else None)
            a = data['acc']
            data['hist'] = [x for x in self.hist if x[0] > a - 3900]
            data['series'] = [x for x in self.series if x[0] > a - 3900]
            tmp = STATE_PATH + '.tmp'
            with open(tmp, 'w', encoding='utf-8') as f:
                json.dump(data, f, ensure_ascii=False)
            os.replace(tmp, STATE_PATH)
            return True
        except Exception:
            log_error(traceback.format_exc())
            return False

    def load_state(self):
        try:
            if not os.path.exists(STATE_PATH):
                return
            with open(STATE_PATH, encoding='utf-8') as f:
                data = json.load(f)
        except Exception:
            log_error(traceback.format_exc())
            return
        try:
            tup = lambda v: tuple(v) if isinstance(v, list) else v
            for k in self.KEEP_KEYS:
                if k in data:
                    v = data[k]
                    if isinstance(v, dict):
                        v = {kk: tup(vv) for kk, vv in v.items()}
                    setattr(self, k, tup(v) if k in ('exp_last',) else v)
            if not data.get('running'):
                return
            for k in self.SESSION_KEYS:
                if k in data:
                    v = data[k]
                    if isinstance(v, dict):
                        v = {kk: tup(vv) for kk, vv in v.items()}
                    setattr(self, k, v)
            ma = tup(self.meso_anchor)
            if ma is not None:
                if len(ma) < 4:
                    ma = tuple(ma) + (0.0, None)[len(ma) - 2:]
                else:
                    ma = (ma[0], ma[1], ma[2], ma[3] if isinstance(ma[3], float) and ma[3] > 1e9 else None)
            self.meso_anchor = ma
            self.hist = [tuple(x) for x in self.hist]
            self.series = [tuple(x) for x in self.series]
            self.acc = float(data.get('acc') or 0)
            self.started_at = datetime.datetime.fromisoformat(data['started_at']) if data.get('started_at') else None
            self.running, self.paused, self.run_start = True, True, 0.0
            away = (time.time() - data.get('saved_at', 0)) / 3600
            if away > RESUME_HOURS:
                saved = self.log_session()
                log_detail(f'上一段（{hms(self.acc)}）隔了 {away:.1f} 小時才開，存進練功紀錄後重新開始')
                self.running = self.paused = False
                self.new_session()
                for k in ('hp', 'mp'):
                    if self.mode(k) == 'bag':
                        self.total[k] = None
                self.save_state()
                self.say('上一段已經存進練功紀錄.csv，這次重新開始' if saved else '上一段太短沒有存，這次重新開始', 8)
            else:
                if not self.seg_pending:
                    self.close_seg(self.acc)
                self.seg_end = None
                log_detail(f'接續上一段（已練 {hms(self.acc)}，暫停中）')
                self.say(f'已接回上一段（{hms(self.acc)}），暫停中，按 ▶ 繼續', 10)
        except Exception:
            log_error(traceback.format_exc())
            self.running = self.paused = False
            self.new_session()

    @staticmethod
    def drop_state():
        try:
            os.remove(STATE_PATH)
        except OSError:
            pass

    def autosave(self):
        self.root.after(60000, self.autosave)
        if self.running:
            self.save_state()

    def new_session(self):
        self.acc = 0.0
        self.run_start = 0.0
        self.started_at = None
        self.gain = 0
        self.hist = []
        self.levelups = self.deaths = 0
        self.qs_used = {'hp': 0, 'mp': 0}
        self.bag_used = {'hp': 0, 'mp': 0}
        self.meso_anchor = None
        self.inc_done = None
        self.seg_pending = False
        self.seg_end = None
        self.pend_dec = None
        self.reject_n = 0
        self.pre_inc = None
        self.rej_run = None
        self.cap_since = {'hp': None, 'mp': None}
        self.series = []
        self.qs_sus_cnt = {'hp': 0, 'mp': 0}
        self.last_drop = {'hp': None, 'mp': None}

    def snapshot(self, a):
        return (a, self.gain, self.used('hp'), self.used('mp'), self.income())

    def recent(self, a, span=600):
        cur = self.snapshot(a)
        if not self.series:
            return cur, None, 0
        base = self.series[0]
        for snap in self.series:
            if snap[0] > a - span:
                break
            base = snap
        return cur, base, a - base[0]

    def toggle_unit(self):
        self.cfg['unit'] = 3600 if self.cfg.get('unit', 600) == 600 else 600
        self.save_cfg()
        self._refresh()

    def active(self):
        return self.acc + (time.time() - self.run_start if self.counting() else 0)

    def counting(self):
        return self.running and not self.paused

    def used(self, k):
        return self.qs_used[k] + self.bag_used[k]

    def stuck(self, k):
        return bool(self.cap_since[k] and time.time() - self.cap_since[k] > 300)

    def mode(self, k):
        return 'qs' if self.cfg.get(k + '_region') and not self.stuck(k) else 'bag'

    def est_total(self, k):
        t = self.total[k]
        if t is None:
            return None
        T, q, a, _ = t
        return max(0, T - (self.qs_all[k] - q) + (self.qs_add[k] - a))

    def inv_ready(self, k=None):
        keys = (k,) if k else ('hp', 'mp')
        return bool(self.cfg.get('inv_region')) and any(os.path.exists(tpl_path(x)) for x in keys)

    def start_meso(self, fresh=60):
        p = self.meso_pend
        if fresh < 60 and p and p[0] != self.meso:
            return
        if self.meso_anchor is None and self.meso is not None and time.time() - self.meso_t < fresh:
            spent = self.spent_all if self.meso_bag == self.cur_bag_id else self.meso_spent
            self.meso_anchor = (self.meso, spent, self.active(), self.meso_t)

    def close_seg(self, act_end):
        ma = self.meso_anchor
        if ma is None or self.meso is None:
            return
        d = self.inc_done or [0, 0, 0.0]
        self.inc_done = [d[0] + self.meso - ma[0], d[1] + self.meso_spent - ma[1], d[2] + max(0.0, act_end - ma[2])]
        self.meso_anchor = None
        self.seg_end = (self.meso, self.meso_t, ma[3])

    def feed_meso(self, v, bag_id=0):
        p = self.meso_pend
        if p and p[0] == v:
            p[1] += 1
            p[2].add(bag_id)
        else:
            p = self.meso_pend = [v, 1, {bag_id}]
        last = self.meso
        big = bool(last) and (abs(v - last) > last * 0.5 or abs(len(str(v)) - len(str(last))) >= 2)
        need = 3 if self.counting() and self.meso_anchor is None else 2
        if p[1] < need or (big and len(p[2]) < 2):
            return 'wait'
        if v != last or self.meso_spent != self.spent_all:
            log_detail(f'楓幣 {v:,}' + (f'（上次 {last:,}，{v - last:+,}）' if last is not None else '（第一次讀到）') +
                       (f'，買藥累計 {self.spent_all:,}' if self.spent_all else ''))
        self.meso = v
        self.meso_spent = self.spent_all
        self.meso_t = time.time()
        self.meso_bag = bag_id
        if self.paused and self.seg_pending:
            self.seg_pending = False
            self.close_seg(self.acc)
            log_detail(f'楓幣 {v:,} 當作暫停前的結尾，暫停期間的楓幣增減不算')
        elif self.counting() and self.meso_anchor is None:
            self.meso_anchor = (v, self.spent_all, self.active(), self.meso_t)
            log_detail(f'楓幣 {v:,} 當作這次的起點')
        return 'ok'

    def income_parts(self):
        d, ma = self.inc_done, self.meso_anchor
        if ma is None or self.meso is None:
            return None if d is None else (d[0], d[1])
        dm, back = self.meso - ma[0], self.meso_spent - ma[1]
        return (dm, back) if d is None else (d[0] + dm, d[1] + back)

    def income(self):
        p = self.income_parts()
        return None if p is None else p[0] + p[1]

    def income_secs(self, a):
        d, ma = self.inc_done, self.meso_anchor
        return (d[2] if d else 0) + (a - ma[2] if ma else 0)

    def feed_exp(self, exp, pct, trusted):
        counting, now = self.counting(), self.active()
        need_est = exp * 100 / pct if pct and pct >= 1 else None

        def accept(add, levelup=False, inc=False):
            if inc:
                self.pre_inc = (self.exp_last[0], len(self.hist), time.time())
            self.exp_last = (exp, pct)
            self.reject_n = 0
            self.rej_run = None
            self.pend_dec = None
            if levelup:
                self.need = need_est
            elif need_est and (pct >= 3 or not self.need):
                self.need = need_est
            if counting:
                self.gain += add
                self.hist.append((now, self.gain))
            return True

        if self.exp_last is None:
            if not trusted and self.pend_first != (exp, pct):
                self.pend_first = (exp, pct)
                return False
            self.pend_first = None
            return accept(0)
        lexp, lpct = self.exp_last
        if exp == lexp:
            if not trusted and self.need and pct is not None and abs(pct - exp * 100 / self.need) > 1:
                pct, need_est = lpct, None
            return accept(0)

        if exp > lexp:
            d = exp - lexp
            if not trusted and self.need:
                ok_need = bool(need_est) and pct >= 3 and abs(need_est - self.need) / self.need <= 0.03
                if need_est and pct >= 3 and not ok_need:
                    return self.reject(exp, pct, need_est, up=True)
                if d > 0.2 * self.need and not (ok_need and lpct is not None
                                                and abs((pct - lpct) - d * 100 / self.need) < 1):
                    return self.reject(exp, pct, need_est, up=True)
            return accept(d, inc=True)

        if pct is not None and lpct is not None:
            lp = lexp * 100 / self.need if self.need else lpct
            levelup = lp - pct > 15
        else:
            levelup = exp < lexp * 0.5
        if not trusted:
            p = self.pend_dec
            if not (p and (pct is None or p[1] is None or abs(p[1] - pct) < 2)):
                self.pend_dec = (exp, pct)
                return self.reject(exp, pct, need_est, keep_pend=True)
            if self.need and need_est and pct >= 3:
                if levelup and need_est < self.need * 0.99:
                    return self.reject(exp, pct, need_est)
                if not levelup and abs(need_est - self.need) / self.need > 0.03:
                    return self.reject(exp, pct, need_est)
        pre = self.pre_inc
        self.pre_inc = None
        if not levelup and pre and exp >= pre[0] and time.time() - pre[2] < 60:
            if counting:
                del self.hist[pre[1]:]
            log_detail(f'EXP {lexp:,} → {exp:,}：上一筆是讀錯，已修正')
            return accept(exp - lexp)
        if levelup:
            if counting:
                self.levelups += 1
            log_detail(f'EXP {lexp:,}[{lpct}%] → {exp:,}[{pct}%]：升級')
            return accept(round(self.need - lexp if self.need else 0) + exp, levelup=True)
        if counting:
            self.deaths += 1
        log_detail(f'EXP {lexp:,}[{lpct}%] → {exp:,}[{pct}%]：死亡，掉了 {lexp - exp:,}')
        return accept(exp - lexp)

    def reject(self, exp, pct, need_est, keep_pend=False, up=False):
        if not keep_pend:
            self.pend_dec = None
        r = self.rej_run
        if up and r and exp >= r[1] and exp - r[1] <= 0.05 * (self.need or exp):
            self.rej_run = (r[0], exp, r[2] + 1)
        else:
            self.rej_run = (exp, exp, 1) if up else None
        self.reject_n += 1
        if self.reject_n >= 5:
            log_detail(f'EXP 連續 5 次對不上（{self.exp_last} → {exp:,}[{pct}%]），重新當基準')
            if up and self.rej_run and self.rej_run[2] >= 5 and self.counting() and self.exp_last:
                lexp, lpct = self.exp_last
                d, n = exp - lexp, need_est or self.need
                if pct is not None and lpct is not None and n and abs((pct - lpct) - d * 100 / n) < 1:
                    self.gain += d
                    self.hist.append((self.active(), self.gain))
            self.exp_last = (exp, pct)
            if need_est:
                self.need = need_est
            self.reject_n = 0
            self.pend_dec = self.rej_run = self.pre_inc = None
        return False

    def qs_use(self, k, n, sus=False):
        c, self.qs_credit[k] = min(n, self.qs_credit[k]), 0
        n -= c
        if n <= 0:
            return
        self.qs_all[k] += n
        if sus or n >= 20:
            self.qs_sus[k] += n
        if self.counting():
            self.qs_used[k] += n
            if sus or n >= 20:
                self.qs_sus_cnt[k] += n

    def feed_qs(self, k, val):
        last, now = self.qs[k], time.time()
        gap, self.qs_seen[k] = now - self.qs_seen[k], now
        if last is None or val == last:
            self.qs[k], self.qs_pend[k], self.qs_t[k] = val, None, now
            self.qs_credit[k] = 0
            return
        step = max(15, (now - self.qs_t[k]) / 60 * 90)
        p = self.qs_pend[k]
        if p and gap > self.blind_sec():
            p = self.qs_pend[k] = None
        if p and val < p[0] < last and last - val <= step:
            self.qs_use(k, last - p[0])
            self.qs[k], self.qs_pend[k], self.qs_t[k] = p[0], [val, 1], now
            return
        if p and p[0] == val:
            p[1] += 1
        else:
            self.qs_pend[k] = [val, 1]
            return
        cap = self.cfg.get('stack_max') or 0
        if val > last:
            if last <= step:
                n = max(0, last + (cap - val if cap and cap - step <= val <= cap else 0))
                self.qs_use(k, n, sus=True)
                log_detail(f'{self.cfg[k + "_name"]} 快捷欄 {last} → {val}（這格用完換下一格，算用掉 {n}）')
            else:
                log_detail(f'{self.cfg[k + "_name"]} 快捷欄 {last} → {val}（變多，不算買藥，等開背包確認）')
        elif last - val <= step:
            self.qs_use(k, last - val)
        elif p[1] < 8:
            return
        else:
            log_detail(f'{self.cfg[k + "_name"]} 快捷欄 {last} → {val}（一下子少太多，當成換格顯示，不算用量）')
        self.qs[k], self.qs_pend[k], self.qs_t[k] = val, None, now
        self.qs_credit[k] = 0

    def qs_miss(self, k, miss, now, slots):
        name, prev = self.cfg[k + '_name'], self.total[k]
        blind_for = now - self.qs_t[k]
        if self.qs[k] is not None and blind_for <= self.blind_sec():
            if miss > 5:
                self.last_drop[k] = (miss, now, 0, 0, slots)
            return
        if prev[3] < self.fill_from or miss > max(15, min(now - prev[3], blind_for) / 60 * 90):
            self.last_drop[k] = (miss, now, 0, 0, slots)
            log_detail(f'{name} 快捷欄讀不到時背包少了 {miss:,}，不像是喝掉的，不算用量')
            return
        self.qs_credit[k] += miss
        self.qs_pend[k] = None
        if self.counting():
            n = miss
        else:
            n = max(0, min(miss, round(miss * (self.active() - self.total_act[k]) / max(1.0, now - prev[3]))))
        self.bag_used[k] += n
        self.last_drop[k] = (miss, now, n, miss, slots)
        if n:
            log_detail(f'{name} 快捷欄讀不到的這段時間用掉 {n:,} 瓶（用背包的數字補算）')

    def blind_sec(self):
        return max(8, 2 * float(self.cfg.get('ocr_sec') or 3) + 2)

    @staticmethod
    def drop_back(ld, slots):
        if len(ld) < 5:
            return True
        s = ld[4]
        return bool(s and s[0] and slots and s[1] < s[0] and slots[1] > s[1]) or (not ld[3] and ld[0] <= 15)

    def set_total(self, k, T, slots=None, seen=None):
        est, now = self.est_total(k), time.time()
        price, name = self.cfg[k + '_price'], self.cfg[k + '_name']
        if est is None:
            log_detail(f'{name} 背包總數 {T:,}（第一次讀到，當作起點）')
        else:
            lr, ld = self.last_rise[k], self.last_drop[k]
            amt = T - est
            if amt > 5 and self.qs_sus[k] >= amt - 5:
                back = min(amt, self.qs_sus_cnt[k])
                self.qs_used[k] -= back
                log_detail(f'{name} 背包總數 {T:,}，預估 {est:,}（多 {amt:,}）→ 快捷欄之前讀錯多扣了，不是買藥'
                           + (f'，退回用量 {back:,}' if back else ''))
            elif amt > 5 and ld and now - ld[1] < 1800 and abs(amt - ld[0]) <= 5 and self.drop_back(ld, slots):
                self.bag_used[k] -= ld[2]
                if len(ld) > 3:
                    self.qs_credit[k] = max(0, self.qs_credit[k] - ld[3])
                self.last_drop[k] = None
                log_detail(f'{name} 背包總數 {T:,}，上次少掉的 {ld[0]:,} 又出現了（當時被擋住）→ 退回用量')
            elif amt > 5:
                cost = amt * price
                self.spent_all += cost
                ma, se = self.meso_anchor, self.seg_end
                adj_a = bool(seen and ma and ma[3] is not None and seen <= ma[3])
                if adj_a:
                    self.meso_anchor = (ma[0], ma[1] + cost, ma[2], ma[3])
                adj_d = bool(seen and self.inc_done and se and (se[2] is None or seen > se[2]) and seen <= se[1])
                if adj_d:
                    self.inc_done[1] += cost
                self.last_rise[k] = (amt, est, now, self.qs_all[k], self.qs_add[k], adj_a, adj_d)
                log_detail(f'{name} 背包總數 {T:,}，預估 {est:,} → 算買了 {amt:,} 瓶（{amt * price:,} 楓幣，收益會加回）')
                self.say(f'偵測到買了 {amt:,} 瓶{name}（{short(amt * price)}），收益會把這筆加回來', 8)
            else:
                if lr and now - lr[2] < 1800:
                    amt, est0, _, q0, a0 = lr[:5]
                    est0_now = est0 - (self.qs_all[k] - q0) + (self.qs_add[k] - a0)
                    if abs(T - est0_now) <= 5 + (0 if self.mode(k) == 'qs' else 30):
                        self.spent_all -= amt * price
                        ma = self.meso_anchor
                        if len(lr) > 5 and lr[5] and ma:
                            self.meso_anchor = (ma[0], ma[1] - amt * price, ma[2], ma[3])
                        if len(lr) > 6 and lr[6] and self.inc_done:
                            self.inc_done[1] -= amt * price
                        self.last_rise[k] = None
                        log_detail(f'{name} 背包總數 {T:,}，回到補貨前的數字 → 上次的「買了 {amt:,} 瓶」是讀錯，退回')
                        est = est0_now
                    elif abs(T - est) <= 5:
                        self.last_rise[k] = None
                if T != est:
                    log_detail(f'{name} 背包總數 {T:,}，預估 {est:,}（差 {T - est:+,}）')
            miss = est - T
            if self.mode(k) == 'bag':
                if self.running and miss > 0:
                    self.bag_used[k] += miss
                    self.last_drop[k] = (miss, now, miss)
            elif miss > 0:
                self.qs_miss(k, miss, now, slots)
        self.total[k] = (T, self.qs_all[k], self.qs_add[k], now)
        self.total_act[k] = self.active()
        self.qs_sus[k] = self.qs_sus_cnt[k] = 0

    def feed_inv(self, k, stacks):
        cap = self.cfg.get('stack_max') or 0
        if any(v is None or (cap and v > cap) for v in stacks):
            return 'bad'
        T, n = sum(stacks), len(stacks)
        p = self.inv_pend[k]
        if p and p[0] == T and p[1] == n:
            p[2] += 1
        else:
            p = self.inv_pend[k] = [T, n, 1, time.time()]
        est = self.est_total(k)
        if est is None:
            need = 3
        elif T > est + 5:
            need = 4
        elif est - T > max(60, (time.time() - self.total[k][3]) / 60 * 40):
            need = 4
            if self.inv_slots[k] and n < self.inv_slots[k] and est - T > 200:
                need = 6
        else:
            need = 2
        if p[2] < need:
            return 'wait'
        slots = (self.inv_slots[k], n)
        self.inv_slots[k] = n
        self.set_total(k, T, slots, seen=p[3] if len(p) > 3 else None)
        return 'ok'

    def stats(self):
        a = self.active()
        h = a / 3600
        s = dict(active=a)
        s['rate'] = self.gain / h if h > 0.005 else None
        span = self.cfg.get('unit', 600)
        s['rate_recent'] = None
        if a >= 120 and len(self.hist) >= 2:
            base = self.hist[0]
            for t, g in self.hist:
                if t > a - span:
                    break
                base = (t, g)
            end_t, end_g = self.hist[-1]
            if end_t - base[0] > 30:
                s['rate_recent'] = (end_g - base[1]) / ((end_t - base[0]) / 3600)
        if len(self.hist) > 2000:
            cut = next((i for i, (t, _) in enumerate(self.hist) if t > a - 3900), 0)
            if cut > 1:
                del self.hist[:cut - 1]
                if self.pre_inc:
                    v, idx, t0 = self.pre_inc
                    self.pre_inc = (v, max(0, idx - (cut - 1)), t0)
        cost = self.used('hp') * self.cfg['hp_price'] + self.used('mp') * self.cfg['mp_price']
        s['cost'] = cost
        s['cost_hr'] = cost / h if h > 0.005 else None
        s['eff'] = self.gain / cost * 1e4 if cost > 0 else None
        for k in ('hp', 'mp'):
            s[k + '_hr'] = self.used(k) / h if h > 0.005 else None
        s['income'] = self.income()
        s['profit'] = None if s['income'] is None else s['income'] - cost
        s['income_hr'] = s['income'] / h if s['income'] is not None and h > 0.005 else None
        s['profit_hr'] = s['profit'] / h if s['profit'] is not None and h > 0.005 else None
        return s

    def build(self):
        r = self.root
        r.overrideredirect(True)
        r.attributes('-topmost', True)
        r.attributes('-alpha', self.cfg['alpha'])
        r.configure(bg=C['line'])
        x, y = int(self.cfg['x']), int(self.cfg['y'])
        try:
            user32.MonitorFromPoint.argtypes = [wintypes.POINT, wintypes.DWORD]
            user32.MonitorFromPoint.restype = wintypes.HANDLE
            if not user32.MonitorFromPoint(wintypes.POINT(x + 30, y + 15), 0):
                x, y = 40, 40
                self.cfg['x'], self.cfg['y'] = x, y
        except Exception:
            pass
        r.geometry(f'+{x}+{y}')
        box = tk.Frame(r, bg=C['bg'], padx=10, pady=8)
        box.pack(padx=1, pady=1)

        head = tk.Frame(box, bg=C['bg'])
        head.pack(fill='x')
        self.dot = tk.Label(head, text='●', bg=C['bg'], fg=C['muted'], font=(FONT, 9))
        self.dot.pack(side='left')
        self.timer = tk.Label(head, text='00:00:00', bg=C['bg'], fg=C['ink'], font=(MONO, 15, 'bold'),
                              width=8, anchor='w')
        self.timer.pack(side='left', padx=(4, 10))
        self.btns = {}
        for key, txt, cmd in (('run', '▶', self.toggle_run), ('rec', '✎', self.open_record),
                              ('set', '⚙', self.open_settings), ('reset', '⟲', self.ask_reset),
                              ('compact', '▭', self.toggle_compact), ('close', '✕', self.quit)):
            b = tk.Label(head, text=txt, bg=C['bg'], fg=C['muted'], font=(FONT, 12), padx=6, pady=2, cursor='hand2')
            b.pack(side='left')
            b.bind('<Button-1>', lambda e, c=cmd: c())
            b.bind('<Enter>', lambda e, b=b, k=key: (b.configure(fg=C['ink']), self.hover(k)))
            b.bind('<Leave>', lambda e, b=b, k=key: (b.configure(fg=C['hp'] if k == 'reset' and self.reset_armed else C['muted']),
                                                     self.hover(None)))
            self.btns[key] = b

        self.compact_lbl = tk.Label(box, text='', bg=C['bg'], fg=C['exp'], font=(MONO, 10, 'bold'), anchor='w')
        self.body = tk.Frame(box, bg=C['bg'])
        self.vals = {}
        self.keys = {}
        self.unit_lbl = tk.Label(self.body, text='', bg=C['bg'], fg=C['mp'], font=(FONT, 8), anchor='w', cursor='hand2')
        self.unit_lbl.grid(row=0, column=0, sticky='w', padx=(0, 10))
        heads = [self.unit_lbl]
        for c, t in ((1, '近10分'), (2, '平均')):
            h = tk.Label(self.body, text=t, bg=C['bg'], fg=C['muted'], font=(FONT, 8), anchor='w', cursor='hand2')
            h.grid(row=0, column=c, sticky='w', padx=(0, 14))
            heads.append(h)
        self.recent_hdr, self.avg_hdr = heads[1], heads[2]
        for h in heads:
            h.bind('<Button-1>', lambda e: self.toggle_unit())
        row = 1
        for k, lab, col in self.PAIR_ROWS:
            kl = tk.Label(self.body, text=lab, bg=C['bg'], fg=C['muted'], font=(FONT, 9), anchor='w')
            kl.grid(row=row, column=0, sticky='w', padx=(0, 10))
            recent = tk.Label(self.body, text='–', bg=C['bg'], fg=C[col], font=(MONO, 11, 'bold'), anchor='w')
            recent.grid(row=row, column=1, sticky='w', padx=(0, 14))
            avg = tk.Label(self.body, text='–', bg=C['bg'], fg=C[col], font=(MONO, 11), anchor='w')
            avg.grid(row=row, column=2, sticky='w')
            self.keys[k], self.vals[k] = kl, (recent, avg)
            row += 1
        tk.Frame(self.body, bg=C['line'], height=1).grid(row=row, column=0, columnspan=3, sticky='ew', pady=4)
        row += 1
        for k, lab, col in self.ONE_ROWS:
            kl = tk.Label(self.body, text=lab, bg=C['bg'], fg=C['muted'], font=(FONT, 9), anchor='w')
            kl.grid(row=row, column=0, sticky='w', padx=(0, 10))
            vl = tk.Label(self.body, text='–', bg=C['bg'], fg=C[col], font=(MONO, 11, 'bold'), anchor='w')
            vl.grid(row=row, column=1, columnspan=2, sticky='w')
            self.keys[k], self.vals[k] = kl, vl
            row += 1
        self.meso_rows_shown = True
        self.status = tk.Frame(box, bg=C['bg'])
        self.status_key, self.status_text = None, ''
        self.layout()

        val_labels = [x for v in self.vals.values() for x in (v if isinstance(v, tuple) else (v,))]
        for w in [box, head, self.timer, self.dot, self.body, self.compact_lbl, self.status] + \
                 val_labels + list(self.keys.values()):
            w.bind('<ButtonPress-1>', self.drag_start, add='+')
            w.bind('<B1-Motion>', self.drag_move, add='+')
            w.bind('<ButtonRelease-1>', self.drag_end, add='+')
        r.after(100, self.no_activate)

    def layout(self):
        for w in (self.compact_lbl, self.body, self.status):
            w.pack_forget()
        if self.cfg['compact']:
            self.compact_lbl.pack(fill='x', pady=(4, 0))
        else:
            self.body.pack(fill='x', pady=(6, 0))
            self.status.pack(fill='x', pady=(6, 0))
        self.status_packed = not self.cfg['compact']

    BTN_HINTS = {
        'run': '開始／暫停計時（Ctrl+F10）',
        'rec': '手動輸入 EXP 或藥水總數：自動讀不到時才需要用（Ctrl+F11）',
        'set': '設定：藥水價格、框選要自動讀取的位置',
        'reset': '結束這一段：成績存進練功紀錄.csv，重新開始算（要連按兩下）',
        'compact': '切換精簡模式（只顯示一行）',
        'close': '關閉浮窗（會先存紀錄）',
    }

    def hover(self, key):
        self.hover_hint = self.BTN_HINTS.get(key) if key else None
        self._refresh()

    @staticmethod
    def alive(w):
        try:
            return w is not None and bool(w.winfo_exists())
        except Exception:
            return False

    def hwnd(self, w=None):
        return user32.GetAncestor((w or self.root).winfo_id(), 2)

    def no_activate(self):
        h = self.hwnd()
        ex = user32.GetWindowLongW(h, -20)
        user32.SetWindowLongW(h, -20, ex | 0x08000000 | 0x00000080)

    def keep_top(self):
        self.root.after(1500, self.keep_top)
        if getattr(self, 'selecting', False):
            return
        try:
            flags = 0x0001 | 0x0002 | 0x0010
            if self.root.state() != 'withdrawn':
                user32.SetWindowPos(self.hwnd(), HWND_TOPMOST, 0, 0, 0, 0, flags)
            wins = [w for w in (self.set_win, self.rec_win) if self.alive(w) and w.state() == 'normal']
            front = getattr(self, '_front_dlg', None)
            wins.sort(key=lambda w: w is front)
            for w in wins:
                user32.SetWindowPos(self.hwnd(w), HWND_TOPMOST, 0, 0, 0, 0, flags)
        except Exception:
            pass

    def force_focus(self, w, entry=None):
        try:
            hwnd = self.hwnd(w)
            fg = user32.GetForegroundWindow()
            if fg and fg != hwnd:
                me = kernel32.GetCurrentThreadId()
                other = user32.GetWindowThreadProcessId(fg, None)
                attached = other and other != me and user32.AttachThreadInput(me, other, True)
                user32.BringWindowToTop(hwnd)
                user32.SetForegroundWindow(hwnd)
                if attached:
                    user32.AttachThreadInput(me, other, False)
        except Exception:
            log_error(traceback.format_exc())
        w.focus_force()
        if entry is not None:
            entry.focus_set()
            entry.icursor('end')

    def set_status(self, msg, col):
        if (msg, col) == self.status_key:
            return
        self.status_key, self.status_text = (msg, col), msg
        for w in self.status.winfo_children():
            w.destroy()
        for line in msg.split('\n'):
            if line.startswith('自動讀取：'):
                segs = [('自動讀取：', C['muted'])]
                for i, part in enumerate(line[len('自動讀取：'):].split('、')):
                    if i:
                        segs.append(('、', C['muted']))
                    bad = '讀不到' in part or '失敗' in part
                    segs.append((part, C['hp'] if bad else C['good'] if '✓' in part or part == 'OK' else C['muted']))
            else:
                segs = [(line, col)]
            row = tk.Frame(self.status, bg=C['bg'])
            row.pack(fill='x', anchor='w')
            for w in (row,) + tuple(tk.Label(row, text=t, bg=C['bg'], fg=c, font=(FONT, 8), padx=0, bd=0)
                                    for t, c in segs):
                if w is not row:
                    w.pack(side='left')
                w.bind('<ButtonPress-1>', self.drag_start, add='+')
                w.bind('<B1-Motion>', self.drag_move, add='+')
                w.bind('<ButtonRelease-1>', self.drag_end, add='+')

    def drag_start(self, e):
        self._drag = (e.x_root - self.root.winfo_x(), e.y_root - self.root.winfo_y())

    def drag_move(self, e):
        dx, dy = self._drag
        self.root.geometry(f'+{e.x_root - dx}+{e.y_root - dy}')

    def drag_end(self, e):
        self.cfg['x'], self.cfg['y'] = self.root.winfo_x(), self.root.winfo_y()
        self.save_cfg()

    def save_cfg(self):
        try:
            with open(CFG_PATH, 'w', encoding='utf-8') as f:
                json.dump(self.cfg, f, ensure_ascii=False, indent=2)
        except Exception:
            log_error(traceback.format_exc())

    def on_error(self, exc, val, tb):
        msg = f'{exc.__name__}: {val}'
        last = getattr(self, '_last_err', (None, 0))
        if msg != last[0] or time.time() - last[1] > 60:
            log_error(''.join(traceback.format_exception(exc, val, tb)))
            self._last_err = (msg, time.time())
        self.say(f'出錯了：{val}（詳細內容在 error.log）', 15)
        if getattr(self, 'selecting', False):
            self.selecting = False
            self.restore_after_select()

    def say(self, text, sec=4):
        self.alert = (text, time.time() + sec)

    def show_win(self, w, entry=None):
        w.deiconify()
        w.attributes('-topmost', True)
        w.lift()
        w.after(30, lambda: self.force_focus(w, entry))

    def place_near(self, w, avoid=None):
        w.update_idletasks()
        frame = 60
        ww, wh = w.winfo_reqwidth() + 16, w.winfo_reqheight() + frame
        rx, ry = self.root.winfo_x(), self.root.winfo_y()
        rw, rh = self.root.winfo_width(), self.root.winfo_height()
        sw, sh = w.winfo_screenwidth(), w.winfo_screenheight() - 50
        rects = [(rx, ry, rw, rh)]
        if self.alive(avoid) and avoid.state() == 'normal':
            rects.append((avoid.winfo_x(), avoid.winfo_y(), avoid.winfo_width() + 16, avoid.winfo_height() + frame))
        side_y = max(0, min(ry, sh - wh))
        spots = [(rx, ry + rh + 8), (rx, ry - wh - 8), (rx + rw + 8, side_y), (rx - ww - 8, side_y)]
        for ax, ay, aw, ah in rects[1:]:
            spots += [(ax + aw + 8, max(0, min(ay, sh - wh))), (ax - ww - 8, max(0, min(ay, sh - wh)))]
        for x, y in spots:
            x = max(0, min(x, sw - ww))
            if y < 0 or y + wh > sh:
                continue
            if all(x >= ax + aw or x + ww <= ax or y >= ay + ah or y + wh <= ay for ax, ay, aw, ah in rects):
                w.geometry(f'+{x}+{y}')
                return
        w.geometry(f'+{max(0, min(rx, sw - ww))}+{max(0, min(ry, sh - wh))}')

    def toggle_run(self):
        if not self.running:
            self.running, self.paused = True, False
            self.run_start = time.time()
            self.started_at = datetime.datetime.now()
            self.fill_from = max(self.fill_from, time.time() - 600)
            self.start_meso()
            for k in ('hp', 'mp'):
                t = self.total[k]
                if self.mode(k) == 'bag' and t and time.time() - t[3] > 600:
                    self.total[k] = None
            need_exp = not self.cfg.get('exp_region') and self.exp_last is None
            no_pot = [k for k in ('hp', 'mp') if self.est_total(k) is None and not self.cfg.get(k + '_region')]
            need_meso = self.cfg.get('meso_region') and self.meso_anchor is None
            self.say('開始計時')
            log_detail('===== 開始計時 =====')
            if (no_pot or need_meso) and self.inv_ready() and not need_exp:
                self.say('開始計時，請打開背包，讓浮窗讀藥水總數和楓幣', 8)
            elif need_exp or no_pot:
                self.say('開始計時，先填入目前的數值當起點', 6)
                self.open_record()
        elif self.paused:
            self.paused = False
            self.run_start = time.time()
            self.cap_since = {'hp': None, 'mp': None}
            no_end = self.seg_pending
            if no_end:
                self.seg_pending = False
            else:
                self.start_meso(fresh=5)
            log_detail('繼續計時')
            if no_end and self.cfg.get('meso_region') and self.meso_anchor is not None:
                self.say('繼續計時（暫停期間沒記錄到楓幣，這段的楓幣增減會計算）', 8)
            elif self.cfg.get('meso_region') and self.meso_anchor is None and self.inc_done is not None:
                self.say('繼續計時，請打開背包確認楓幣', 8)
            else:
                self.say('繼續計時')
        else:
            self.acc += time.time() - self.run_start
            self.paused = True
            log_detail('暫停')
            if self.meso_anchor is not None:
                if time.time() - self.meso_t < 5:
                    self.close_seg(self.acc)
                else:
                    self.seg_pending = True
            if self.seg_pending:
                self.say('已暫停，請打開背包記錄楓幣，暫停期間的楓幣增減不計算', 8)
            else:
                self.say('已暫停，暫停期間的經驗、藥水' + ('、楓幣' if self.cfg.get('meso_region') else '') + '都不計算')

    def ask_reset(self):
        if time.time() - self.reset_armed < 3:
            self.reset_armed = 0.0
            self.btns['reset'].configure(fg=C['muted'])
            saved = self.log_session()
            if saved is False:
                self.say('寫不進練功紀錄（Excel 開著嗎？），這次沒有清除。關掉檔案再按兩下 ⟲', 10)
                return
            s = self.stats()
            log_detail(f'===== 結束這一段：EXP {self.gain:,}、收益 {s["profit"] if s["profit"] is not None else "–"} =====')
            self.running = self.paused = False
            self.new_session()
            for k in ('hp', 'mp'):
                if self.mode(k) == 'bag':
                    self.total[k] = None
            self.save_state()
            if saved is None:
                self.say('已清除（不到 1 分鐘，沒有存）')
            elif saved == LOG_PATH:
                self.say('已清除，本次成績存進 練功紀錄.csv')
            else:
                self.say(f'練功紀錄.csv 被占用，改存到 {os.path.basename(saved)}', 8)
        else:
            self.reset_armed = time.time()
            self.btns['reset'].configure(fg=C['hp'])
            self.say('3 秒內再按一次 ⟲ 就清除本次紀錄', 3)

    def toggle_compact(self):
        self.cfg['compact'] = not self.cfg['compact']
        self.layout()
        self.save_cfg()

    def toggle_visible(self):
        if getattr(self, 'selecting', False):
            self._root_was_hidden = not getattr(self, '_root_was_hidden', False)
            return
        if self.root.state() == 'withdrawn':
            self.root.deiconify()
            self.root.after(50, self.no_activate)
        else:
            self.root.withdraw()

    def quit(self):
        if not self.save_state() and time.time() - getattr(self, 'quit_armed', 0) >= 5:
            if self.log_session() is False:
                self.quit_armed = time.time()
                self.say('存不了進度，也寫不進練功紀錄（Excel 開著嗎？）。5 秒內再按一次 ✕ 就直接關閉', 5)
                return
            self.drop_state()
        if self.running:
            log_detail(f'關閉浮窗，進度已存（{hms(self.active())}）')
        self.save_cfg()
        self.root.destroy()

    def log_session(self):
        a = self.active()
        if a < 60:
            return None
        try:
            s = self.stats()
            header = ['開始時間', '練功分鐘', '獲得EXP', 'EXP每小時', '升級次數', '死亡次數',
                      self.cfg['hp_name'] + '用量', self.cfg['mp_name'] + '用量',
                      '藥水花費', '花費每小時', '每萬楓幣EXP', '撿到楓幣', '淨賺楓幣', '淨賺每小時']
            row = [(self.started_at or datetime.datetime.now()).strftime('%Y-%m-%d %H:%M'),
                   round(a / 60, 1), self.gain, round(s['rate'] or 0), self.levelups, self.deaths,
                   self.used('hp'), self.used('mp'), s['cost'], round(s['cost_hr'] or 0),
                   round(s['eff'] or 0), s['income'] if s['income'] is not None else '',
                   s['profit'] if s['profit'] is not None else '',
                   round(s['profit_hr']) if s['profit_hr'] is not None else '']
        except Exception:
            log_error(traceback.format_exc())
            return False
        base, ext = os.path.splitext(LOG_PATH)
        for path in (LOG_PATH, f'{base}_{datetime.datetime.now():%Y%m%d_%H%M%S}{ext}'):
            new = not os.path.exists(path)
            try:
                if not new:
                    try:
                        with open(path, encoding='utf-8-sig', newline='') as f:
                            rows = list(csv.reader(f))
                    except UnicodeDecodeError:
                        rows = None
                    if rows and len(rows[0]) < len(header):
                        with open(path, 'w', encoding='utf-8-sig', newline='') as f:
                            csv.writer(f).writerows([header] + rows[1:])
                with open(path, 'a', newline='', encoding='utf-8-sig') as f:
                    w = csv.writer(f)
                    if new:
                        w.writerow(header)
                    w.writerow(row)
                return path
            except Exception:
                log_error(traceback.format_exc())
        return False

    def poll(self):
        self.root.after(100, self.poll)
        if self.quit_evt and kernel32.WaitForSingleObject(self.quit_evt, 0) == 0:
            if self.counting() and self.meso_anchor is not None and time.time() - self.meso_t >= 5:
                self.seg_pending = True
            if not self.save_state():
                if self.log_session() is False:
                    log_error('換新浮窗時存不了進度，也寫不進練功紀錄')
                else:
                    self.drop_state()
            if self.running:
                log_detail(f'換新版浮窗，進度已存（{hms(self.active())}）')
            self.save_cfg()
            self.root.destroy()
            return
        while True:
            try:
                kind, data = self.q.get_nowait()
            except queue.Empty:
                return
            try:
                if kind == 'ocr':
                    self.on_ocr(data)
                elif kind == 'ocr_ready':
                    self.ocr_msg = '讀取引擎就緒'
                elif kind == 'ocr_err':
                    self.ocr_msg = data
                    if data.startswith('OCR 無法載入'):
                        self.ocr_fatal = data
                elif kind == 'hotkey_fail':
                    self.hotkey_msg = '、'.join(data) + ' 被其他程式占用，請直接點浮窗上的按鈕'
                elif kind == 'hotkey':
                    {'pause': self.toggle_run, 'toggle': self.toggle_visible, 'record': self.open_record}[data]()
            except Exception as e:
                self.on_error(type(e), e, e.__traceback__)

    def on_ocr(self, out):
        self.ocr_seen = time.time()
        parts = []
        game_visible = True
        self.cur_bag_id = out.get('bag_id', self.cur_bag_id)
        if 'exp' in out:
            self.raw['exp'] = out['exp']
            p = parse_exp(out['exp'] or '')
            if p is not None:
                self.exp_ok_t = time.time()
            game_visible = p is not None or time.time() - self.exp_ok_t > 1800
            if out.get('exp_err'):
                parts.append('EXP 截圖失敗（螢幕鎖定或被擋住）')
            elif p is None:
                parts.append('EXP 讀不到')
            elif self.feed_exp(p[0], p[1], trusted=False):
                parts.append('EXP ✓')
            else:
                parts.append('EXP 確認中')
        for k in ('hp', 'mp'):
            if k in out:
                self.raw[k] = out[k]
                v = parse_count(out[k] or '')
                if out.get(k + '_err'):
                    parts.append(self.cfg[k + '_name'] + ' 截圖失敗')
                elif v is None:
                    parts.append(self.cfg[k + '_name'] + ' 讀不到')
                elif not game_visible:
                    pass
                else:
                    self.feed_qs(k, v)
                cap = self.cfg.get('stack_max')
                if cap and self.counting() and self.qs[k] == cap:
                    self.cap_since[k] = self.cap_since[k] or time.time()
                else:
                    self.cap_since[k] = None
        inv = out.get('inv')
        results = []
        if inv is not None:
            info = []
            bag_open = any(inv.values())
            for k in ('hp', 'mp'):
                if k not in inv:
                    if bag_open and self.cfg.get('inv_region') and not self.inv_ready(k):
                        info.append(f'{self.cfg[k + "_name"]} 還沒框背包裡的一格，讀不到')
                    continue
                stacks = inv[k]
                name = self.cfg[k + '_name']
                if not stacks:
                    qs_has = self.cfg.get(k + '_region') and (self.qs[k] or 0) > 0
                    if bag_open and not qs_has and self.total[k] is not None:
                        r = self.feed_inv(k, [])
                        results.append(r)
                        info.append(f'{name} 沒看到，當成用完了' + ('（確認中）' if r == 'wait' else ''))
                    else:
                        info.append(f'{name} 沒看到')
                    continue
                r = self.feed_inv(k, stacks)
                results.append(r)
                if r == 'bad':
                    info.append(f'{name} {len(stacks)}格（有格子數字讀不到，一直這樣就重框「背包裡一格{name}」）')
                else:
                    info.append(f'{name} {len(stacks)}格 共{sum(stacks):,}' + ('（確認中）' if r == 'wait' else ''))
            self.inv_msg = '｜'.join(info)
        if 'meso' in out:
            self.raw['meso'] = out['meso']
            v = parse_meso(out['meso'] or '')
            if v is not None:
                results.append(self.feed_meso(v, out.get('bag_id', 0)))
        if results:
            parts.append('背包 ✓' if 'ok' in results and 'wait' not in results else '背包讀取中…請保持打開')
        self.ocr_msg = '自動讀取：' + ('、'.join(parts) if parts else 'OK')

    def refresh(self):
        self.root.after(500, self.refresh)
        try:
            self._refresh()
        except Exception as e:
            self.on_error(type(e), e, e.__traceback__)
            try:
                self.set_status(f'出錯了：{e}（詳細內容在 error.log）', C['hp'])
            except Exception:
                pass

    def _refresh(self):
        s = self.stats()
        a = s['active']
        self.timer.configure(text=hms(a))
        self.dot.configure(fg=C['good'] if self.counting() else C['warn'] if self.running else C['muted'])
        self.btns['run'].configure(text='⏸' if self.counting() else '▶')

        if self.counting() and (not self.series or a - self.series[-1][0] >= 5):
            self.series.append(self.snapshot(a))
            if len(self.series) > 1000:
                keep = next((i for i, snap in enumerate(self.series) if snap[0] > a - 3900), 0)
                if keep > 1:
                    del self.series[:keep - 1]

        v = self.vals
        unit = self.cfg.get('unit', 600)
        uname = '每10分' if unit == 600 else '每小時'
        self.unit_lbl.configure(text=f'{uname} ⇄')
        span_name = '10分' if unit == 600 else '1小時'
        early = a < unit
        if early:
            self.recent_hdr.configure(text=f'實際（{dur(a)}）')
            self.avg_hdr.configure(text=f'預估{span_name}')
        else:
            self.recent_hdr.configure(text=f'近{span_name}')
            self.avg_hdr.configure(text='平均')
        hp_p, mp_p = self.cfg['hp_price'], self.cfg['mp_price']

        def per(delta, secs):
            return None if delta is None or secs < 60 else delta / secs * unit

        cur, base, dt = self.recent(a, span=unit)
        d = [None if base is None or c is None or b is None else c - b
             for c, b in zip(cur, base or cur)]
        r_hp, r_mp = per(d[2], dt), per(d[3], dt)
        r_cost = None if r_hp is None else r_hp * hp_p + r_mp * mp_p
        r_inc = per(d[4], dt)
        a_hp, a_mp = per(self.used('hp'), a), per(self.used('mp'), a)
        a_cost = per(s['cost'], a)
        a_inc = per(s['income'], self.income_secs(a))
        rows = {
            'exp': (None if s['rate_recent'] is None else s['rate_recent'] * unit / 3600,
                    None if s['rate'] is None else s['rate'] * unit / 3600),
            'hp': (r_hp, a_hp), 'mp': (r_mp, a_mp), 'cost': (r_cost, a_cost),
            'income': (r_inc, a_inc),
            'profit': (None if r_inc is None or r_cost is None else r_inc - r_cost,
                       None if a_inc is None or a_cost is None else a_inc - a_cost),
        }
        rates = dict(rows)
        if early and self.running:
            actual = {'exp': self.gain, 'hp': self.used('hp'), 'mp': self.used('mp'), 'cost': s['cost'],
                      'income': s['income'], 'profit': s['profit']}
            rows = {k: (actual[k], av) for k, (_, av) in rows.items()}
        waiting = '累積中' if self.running else '–'
        for k, (rv, av) in rows.items():
            for col, (lbl, val) in enumerate(zip(v[k], (rv, av))):
                if col == 0 and early and self.running and k in ('hp', 'mp') and val is not None:
                    lbl.configure(text=f'{val:,.0f}')
                    continue
                if val is None:
                    if k not in ('income', 'profit') or self.income() is not None:
                        txt = waiting
                    elif not self.inv_ready():
                        txt = '先框背包'
                    else:
                        txt = '開背包更新'
                elif k == 'exp':
                    txt = short(val) + (f' {val * 100 / self.need:.2f}%' if self.need else '')
                elif k in ('hp', 'mp'):
                    txt = f'{val:.1f}' if val < 10 else f'{val:,.0f}'
                else:
                    txt = short(val)
                lbl.configure(text=txt)
            if k == 'profit':
                for lbl, val in zip(v[k], (rv, av)):
                    lbl.configure(fg=C['hp'] if val is not None and val < 0 else C['good'])
        for k in ('hp', 'mp'):
            self.keys[k].configure(text=self.cfg[k + '_name'])
        show_meso = bool(self.cfg.get('meso_region'))
        if show_meso != self.meso_rows_shown:
            for k in ('income', 'profit'):
                for w in (self.keys[k],) + v[k]:
                    w.grid() if show_meso else w.grid_remove()
            self.meso_rows_shown = show_meso

        extra = f'  升{self.levelups}級' if self.levelups else ''
        v['gain'].configure(text=f'{self.gain:,}{extra}')
        if s['profit'] is not None:
            dm, back = self.income_parts()
            parts = f"楓幣{'+' if dm >= 0 else '−'}{short(abs(dm))}"
            if back:
                parts += f"、買藥加回{short(back)}"
            v['earn'].configure(text=f"{short(s['profit'])}（{parts}、藥水−{short(s['cost'])}）",
                                fg=C['good'] if s['profit'] >= 0 else C['hp'])
        elif self.cfg.get('meso_region'):
            v['earn'].configure(text=f"藥水花了 {short(s['cost'])}（開背包讀楓幣後算收益）", fg=C['muted'])
        else:
            v['earn'].configure(text=f"藥水花了 {short(s['cost'])}（⚙ 框選背包楓幣就能算收益）", fg=C['muted'])
        lv_txt, lv_time = '–', None
        if self.exp_last and self.need:
            exp, pct = self.exp_last
            rate = s['rate_recent'] or s['rate']
            lv_time = (self.need - exp) / rate * 3600 if rate and rate > 0 else None
            lv_txt = f'{dur(lv_time)}  {exp * 100 / self.need:.2f}%'
        elif self.exp_last:
            lv_txt = f'{self.exp_last[0]:,}'
        v['lv'].configure(text=lv_txt)
        for k in ('hp', 'mp'):
            self.keys['left_' + k].configure(text=self.cfg[k + '_name'] + '剩')
            left = self.est_total(k)
            hr = s[k + '_hr']
            if left is not None:
                txt = f'{left:,}' + (f'  預計可用 {dur(left / hr * 3600)}' if hr else '')
            elif self.qs[k] is not None:
                txt = f'快捷欄 {self.qs[k]:,}（開背包看總數）'
            else:
                txt = '–'
            v['left_' + k].configure(text=txt)
        exp_r, exp_a = rates['exp']
        rows = rates
        exp_txt = f"EXP {short(exp_a)}（預估{span_name}）" if early else f"EXP {short(exp_r)}｜{short(exp_a)}（{uname}）"
        self.compact_lbl.configure(
            text=f"{exp_txt}│ 花{short(rows['cost'][1])}" +
                 (f" │ 淨{short(rows['profit'][1])}" if rows['profit'][1] is not None else '') +
                 f" │ 升級 {dur(lv_time)}")

        msg, col = self.ocr_msg, C['muted']
        ocr_bad = msg.startswith('自動讀取：') and any(w in msg for w in ('讀不到', '失敗'))
        plain = True
        if self.ocr_seen and time.time() - self.ocr_seen > 20 and any(
                self.cfg.get(k + '_region') for k in ('exp', 'hp', 'mp', 'inv')):
            msg, col, plain = '自動讀取停住了，按 ⚙ 檢查', C['hp'], False
        if getattr(self, 'ocr_fatal', None):
            msg, col, plain = self.ocr_fatal + '\n（自動讀取都不能用，詳細內容在 error.log）', C['hp'], False
        elif not self.cfg.get('exp_region'):
            msg, col, plain = '① 按 ⚙ 改藥水名稱、價格，並「框選」畫面下方的 EXP 數字', C['warn'], False
            if not self.running:
                msg += '\n② 按 ▶ 開始計時（Ctrl+F10）'
        elif not self.running:
            msg = '按 ▶ 開始計時（Ctrl+F10）\n' + msg
        for k in ('hp', 'mp'):
            if self.counting() and self.cap_since[k] and time.time() - self.cap_since[k] > 300:
                name = self.cfg[k + '_name']
                msg += (f'\n{name}快捷欄一直停在 {self.cfg["stack_max"]}，即時看不到用量，'
                        f'\n請打開背包更新')
                col, plain = C['warn'], False
        if self.running and self.cfg.get('meso_region'):
            if self.paused and self.seg_pending:
                msg, col, plain = msg + '\n暫停中：請打開背包確認楓幣', C['warn'], False
            elif self.counting() and self.meso_anchor is None and self.inc_done is not None:
                msg, col, plain = msg + '\n請打開背包確認楓幣', C['warn'], False
        if self.hotkey_msg:
            msg, plain = msg + '\n' + self.hotkey_msg, False
        if self.alert and time.time() < self.alert[1]:
            msg, col = self.alert[0], C['good'] if not self.alert[0].startswith('出錯') else C['hp']
            plain = False
        if getattr(self, 'hover_hint', None):
            msg, col, plain = self.hover_hint, C['mp'], False
        self.set_status(msg, col)
        if self.cfg['compact']:
            want = (not plain and col != C['muted']) or ocr_bad
            if want and not self.status_packed:
                self.status.pack(fill='x', pady=(4, 0))
            elif not want and self.status_packed:
                self.status.pack_forget()
            self.status_packed = want

        if self.reset_armed and time.time() - self.reset_armed >= 3:
            self.reset_armed = 0.0
            self.btns['reset'].configure(fg=C['muted'])

    def entry(self, parent, width=14):
        return tk.Entry(parent, width=width, bg=C['panel'], fg=C['ink'], insertbackground=C['ink'],
                        relief='flat', font=(MONO, 12), highlightthickness=1,
                        highlightbackground=C['line'], highlightcolor=C['mp'])

    def open_record(self):
        if getattr(self, 'selecting', False):
            return
        if self.alive(self.rec_win):
            self.show_win(self.rec_win)
            return
        fg = user32.GetForegroundWindow()
        mine = {self.hwnd()} | ({self.hwnd(self.set_win)} if self.alive(self.set_win) else set())
        self.prev_fg = fg if fg and fg not in mine else None
        w = self.rec_win = tk.Toplevel(self.root)
        w.title('記錄目前數值')
        w.attributes('-topmost', True)
        w.configure(bg=C['bg'], padx=14, pady=12)
        w.resizable(False, False)
        rows = [('exp', 'EXP', 'exp'), ('pct', 'EXP %', 'exp'),
                ('hp', self.cfg['hp_name'] + ' 總數', 'hp'), ('mp', self.cfg['mp_name'] + ' 總數', 'mp')]
        ents, first = {}, None
        for i, (k, lab, col) in enumerate(rows):
            tk.Label(w, text=lab, bg=C['bg'], fg=C[col], font=(FONT, 10, 'bold')).grid(row=i, column=0, sticky='w', pady=3)
            e = self.entry(w, 18)
            e.grid(row=i, column=1, pady=3, padx=(10, 0))
            if k in ('exp', 'pct') and self.cfg.get('exp_region'):
                e.configure(state='disabled', disabledbackground=C['bg'])
                tk.Label(w, text='自動讀取中', bg=C['bg'], fg=C['muted'], font=(FONT, 8)).grid(row=i, column=2, padx=(6, 0))
            else:
                if k in ('hp', 'mp') and self.inv_ready():
                    tk.Label(w, text='開背包會自動讀', bg=C['bg'], fg=C['muted'], font=(FONT, 8)).grid(row=i, column=2, padx=(6, 0))
                if first is None:
                    first = e
            ents[k] = e
        hint = '背包裡有好幾格就用 + 加起來，例如 3000+3000+1184\n空白的欄位會跳過。Enter 送出、Esc 取消。'
        if not self.cfg.get('exp_region') and self.exp_last and not self.need:
            hint = '剛升級過，這次請把 EXP % 也填上\n' + hint
        tk.Label(w, text=hint, bg=C['bg'], fg=C['muted'], font=(FONT, 8), justify='left').grid(
            row=4, column=0, columnspan=3, sticky='w', pady=(6, 0))
        self.rec_msg = tk.Label(w, text='', bg=C['bg'], fg=C['hp'], font=(FONT, 9, 'bold'), justify='left')
        self.rec_msg.grid(row=5, column=0, columnspan=3, sticky='w')
        tk.Button(w, text='記錄', command=lambda: self.submit_record(ents), bg=C['mp'], fg='#0b1426',
                  relief='flat', font=(FONT, 10, 'bold'), padx=16).grid(row=6, column=0, columnspan=3, sticky='e', pady=(8, 0))
        w.bind('<Return>', lambda e: self.submit_record(ents))
        w.bind('<Escape>', lambda e: self.close_record())
        w.protocol('WM_DELETE_WINDOW', self.close_record)
        self.place_near(w, avoid=self.set_win)
        w.bind('<FocusIn>', lambda e, w=w: setattr(self, '_front_dlg', w), add='+')
        self.set_ime(w, False)
        for e in ents.values():
            self.set_ime(e, False)
        self.rec_ents = ents
        target = first or ents['exp']
        w.after(30, lambda: self.force_focus(w, target))

    def set_ime(self, widget, on):
        try:
            imm32.ImmAssociateContextEx(widget.winfo_id(), None, 0x10 if on else 0)
        except Exception:
            pass

    @staticmethod
    def parse_input(text, as_float=False):
        t = text.replace(',', '').replace('，', '').replace('%', '').replace(' ', '')
        t = t.replace('x', '*').replace('X', '*').replace('×', '*').replace('＋', '+')
        if not t:
            return None
        if not re.fullmatch(r'\d+(?:\.\d+)?(?:[+*]\d+(?:\.\d+)?)*', t):
            return False
        total = 0.0
        for term in t.split('+'):
            prod = 1.0
            for x in term.split('*'):
                prod *= float(x)
            total += prod
        return total if as_float else int(round(total))

    def submit_record(self, ents):
        vals, bad = {}, []
        for k, e in ents.items():
            if str(e['state']) == 'disabled':
                vals[k] = None
                continue
            v = self.parse_input(e.get(), as_float=(k == 'pct'))
            if v is False or (k == 'pct' and v is not None and not 0 <= v <= 100):
                bad.append(k)
                e.configure(highlightbackground=C['hp'], highlightthickness=2)
            else:
                vals[k] = v
                e.configure(highlightbackground=C['line'], highlightthickness=1)
        if bad:
            self.rec_msg.configure(text='紅框的數字看不懂。輸入法在中文模式的話，按 Shift 切成英數再打')
            ents[bad[0]].focus_set()
            return
        exp, pct, hp, mp = vals['exp'], vals['pct'], vals['hp'], vals['mp']
        if exp is not None and pct is None and self.exp_last and exp < self.exp_last[0]:
            ents['pct'].configure(highlightbackground=C['hp'], highlightthickness=2)
            self.rec_msg.configure(text=f'EXP 比上次（{self.exp_last[0]:,}）少，是升級了嗎？請一起填 EXP %')
            ents['pct'].focus_set()
            return
        if exp is not None:
            if pct is None and self.need and not (self.exp_last and exp < self.exp_last[0]):
                pct = exp * 100 / self.need
            self.feed_exp(exp, pct, trusted=True)
        for k, val in (('hp', hp), ('mp', mp)):
            if val is not None:
                self.set_total(k, val)
        self.close_record()

    def close_record(self):
        if self.alive(self.rec_win):
            self.rec_win.destroy()
            self.rec_win = None
        try:
            if self.prev_fg:
                user32.SetForegroundWindow(self.prev_fg)
        except Exception:
            pass
        self.prev_fg = None

    def open_settings(self):
        if self.set_win and self.set_win.winfo_exists():
            self.show_win(self.set_win)
            return
        win = self.set_win = tk.Toplevel(self.root)
        win.title('經驗收益計算器設定')
        win.attributes('-topmost', True)
        win.configure(bg=C['bg'])
        win.resizable(False, False)
        bar = tk.Frame(win, bg=C['bg'], padx=14, pady=10)
        bar.pack(side='bottom', fill='x')
        cvs = tk.Canvas(win, bg=C['bg'], highlightthickness=0)
        w = tk.Frame(cvs, bg=C['bg'], padx=14, pady=12)
        cvs.create_window((0, 0), window=w, anchor='nw')
        lab = lambda t, r, c=0, col='muted', columnspan=1: tk.Label(w, text=t, bg=C['bg'], fg=C[col], font=(FONT, 9)).grid(
            row=r, column=c, columnspan=columnspan, sticky='w', pady=2)
        fields = [('hp_name', '紅水名稱'), ('hp_price', '紅水單價'), ('hp_heal', '紅水回復 HP'),
                  ('mp_name', '藍水名稱'), ('mp_price', '藍水單價'), ('mp_heal', '藍水回復 MP'),
                  ('stack_max', '快捷欄一格上限'), ('ocr_sec', '自動讀取間隔（秒）')]
        ents = {}
        for i, (k, t) in enumerate(fields):
            lab(t, i)
            e = self.entry(w, 12)
            e.insert(0, str(self.cfg[k]))
            e.grid(row=i, column=1, sticky='w', padx=(10, 0), pady=2)
            is_name = k.endswith('_name')
            e.bind('<FocusIn>', lambda ev, on=is_name: (self.set_ime(win, on), self.set_ime(ev.widget, on)))
            ents[k] = e
        r = len(fields)
        lab('浮窗透明度', r)
        alpha = tk.Scale(w, from_=0.4, to=1.0, resolution=0.02, orient='horizontal', length=160, showvalue=False,
                         bg=C['bg'], fg=C['ink'], troughcolor=C['panel'], highlightthickness=0,
                         command=lambda v: self.root.attributes('-alpha', float(v)))
        alpha.set(self.cfg['alpha'])
        alpha.grid(row=r, column=1, sticky='w', padx=(10, 0))

        hp, mp = self.cfg['hp_name'], self.cfg['mp_name']
        sections = [
            ('自動讀取 EXP', None, [('exp', '下方 EXP 數字', 'exp')]),
            ('藥水即時扣量（快捷欄上的數字）', None,
             [('hp', f'{hp} 數量', 'hp'), ('mp', f'{mp} 數量', 'mp')]),
            ('藥水總數校正（打開背包時自動讀）', '請先打開背包、切到消耗欄，再按下面的「框選」',
             [('inv', '背包範圍', 'ink'), ('hp_tpl', f'背包裡一格{hp}', 'hp'), ('mp_tpl', f'背包裡一格{mp}', 'mp'),
              ('meso', '背包裡的楓幣數字', 'exp')]),
        ]
        self.region_lbls = {}
        rr = r + 1
        for title, hint, items in sections:
            tk.Label(w, text=title, bg=C['bg'], fg=C['ink'], font=(FONT, 10, 'bold')).grid(
                row=rr, column=0, columnspan=3, sticky='w', pady=(12, 2))
            rr += 1
            if hint:
                lab(hint, rr, col='warn', columnspan=3)
                rr += 1
            for k, t, col in items:
                lab(t, rr, col=col)
                f = tk.Frame(w, bg=C['bg'])
                f.grid(row=rr, column=1, columnspan=2, sticky='w', padx=(10, 0))
                for txt, cmd in (('框選', lambda k=k: self.select_region(k)), ('清除', lambda k=k: self.clear_region(k))):
                    tk.Button(f, text=txt, command=cmd, bg=C['panel'], fg=C['ink'], relief='flat',
                              font=(FONT, 9), padx=8).pack(side='left', padx=(0, 4))
                lb = tk.Label(f, text='', bg=C['bg'], fg=C['muted'], font=(MONO, 9), anchor='w', width=34)
                lb.pack(side='left', padx=(4, 0))
                self.region_lbls[k] = lb
                rr += 1
        self.inv_lbl = tk.Label(w, text='', bg=C['bg'], fg=C['good'], font=(FONT, 9), anchor='w', justify='left')
        self.inv_lbl.grid(row=rr, column=0, columnspan=3, sticky='w', pady=(4, 0))
        rr += 1
        tk.Label(w, text='框選「一格」時：從藥水圖示上緣，框到下面數字的下緣，左右框滿那一格。',
                 bg=C['bg'], fg=C['muted'], font=(FONT, 8), justify='left').grid(row=rr, column=0, columnspan=3, sticky='w', pady=(10, 0))

        set_msg = tk.Label(bar, text='', bg=C['bg'], fg=C['hp'], font=(FONT, 9, 'bold'))
        set_msg.pack(side='left')

        def save(closing=False):
            new, bad = {}, []
            for k, e in ents.items():
                t = e.get().strip()
                if k.endswith('_name'):
                    new[k] = t or DEFAULT[k]
                    continue
                n = self.parse_input(t, as_float=True)
                if n is None:
                    continue
                if n is False or n <= 0:
                    bad.append(k)
                    e.configure(highlightbackground=C['hp'], highlightthickness=2)
                else:
                    new[k] = int(n) if n == int(n) else n
                    e.configure(highlightbackground=C['line'], highlightthickness=1)
            if bad and not closing:
                set_msg.configure(text='紅框的數字看不懂（要大於 0）。按 Shift 切成英數再打')
                return
            self.cfg.update(new)
            self.cfg['alpha'] = float(alpha.get())
            self.save_cfg()
            self.ocr.wake.set()
            win.destroy()
            self.set_win = None
        tk.Button(bar, text='儲存', command=save, bg=C['mp'], fg='#0b1426', relief='flat',
                  font=(FONT, 10, 'bold'), padx=18).pack(side='right')
        win.protocol('WM_DELETE_WINDOW', lambda: save(closing=True))
        w.update_idletasks()
        cw, ch = w.winfo_reqwidth(), w.winfo_reqheight()
        self.inv_lbl.configure(wraplength=cw - 28)
        maxh = win.winfo_screenheight() - bar.winfo_reqheight() - 160
        cvs.configure(width=cw, height=min(ch, maxh), scrollregion=(0, 0, cw, ch))
        w.bind('<Configure>', lambda e: cvs.configure(width=e.width, height=min(e.height, maxh),
                                                      scrollregion=(0, 0, e.width, e.height)))
        if ch > maxh:
            sb = tk.Scrollbar(win, orient='vertical', command=cvs.yview)
            sb.pack(side='right', fill='y')
            cvs.configure(yscrollcommand=sb.set)
            win.bind('<MouseWheel>', lambda e: cvs.yview_scroll(int(-e.delta / 120), 'units'))
        cvs.pack(side='left', fill='both', expand=True)
        self.place_near(win, avoid=self.rec_win)
        win.bind('<FocusIn>', lambda e: setattr(self, '_front_dlg', win), add='+')
        self.show_win(win)
        self.update_region_lbls()

    def update_region_lbls(self):
        if not (self.set_win and self.set_win.winfo_exists()):
            return
        parse = {'exp': parse_exp, 'hp': parse_count, 'mp': parse_count, 'meso': parse_meso}
        for k, lb in self.region_lbls.items():
            if k.endswith('_tpl'):
                ok = os.path.exists(tpl_path(k[:2]))
                txt = '已記住' if ok else '未設定'
            elif k == 'inv':
                ok = bool(self.cfg.get('inv_region'))
                txt = '已設定' if ok else '未設定'
            elif self.cfg.get(k + '_region'):
                raw = (self.raw.get(k) or '').strip()
                ok = not raw or parse[k](raw) is not None
                txt = f'讀到：{raw[:26]}' if raw else '已設定，讀取中…'
            else:
                ok, txt = False, '未設定'
            lb.configure(text=txt, fg=C['good'] if ok else C['hp'])
        if self.inv_ready():
            self.inv_lbl.configure(text='背包：' + ('\n　　　'.join(self.inv_msg.split('｜')) if self.inv_msg
                                                  else '還沒看到，請打開背包（消耗欄）'))
        else:
            self.inv_lbl.configure(text='')
        self.set_win.after(1000, self.update_region_lbls)

    def forget_potion(self, k):
        self.total[k] = self.inv_pend[k] = self.inv_slots[k] = None
        self.last_rise[k] = self.last_drop[k] = None
        self.qs_credit[k] = self.qs_sus[k] = 0

    def clear_region(self, k):
        if k.endswith('_tpl'):
            try:
                os.remove(tpl_path(k[:2]))
            except OSError:
                pass
            self.forget_potion(k[:2])
        else:
            self.cfg[k + '_region'] = None
            self.raw[k] = ''
            self.save_cfg()
        if k in ('hp', 'mp'):
            self.cap_since[k] = None
            self.qs[k] = self.qs_pend[k] = None
        if k == 'exp':
            self.exp_last = self.need = self.pend_first = None
        if k in ('inv', 'hp_tpl', 'mp_tpl'):
            self.inv_msg = ''

    def select_region(self, kind):
        if getattr(self, 'selecting', False):
            return
        self.selecting = True
        self._root_was_hidden = self.root.state() == 'withdrawn'
        self.root.withdraw()
        if self.alive(self.set_win):
            self.set_win.withdraw()
        self._rec_hidden = self.alive(self.rec_win) and self.rec_win.state() != 'withdrawn'
        if self._rec_hidden:
            self.rec_win.withdraw()
        self.root.after(300, lambda: self._select(kind))

    def restore_after_select(self):
        self.selecting = False
        if not getattr(self, '_root_was_hidden', False):
            self.root.deiconify()
            self.root.after(50, self.no_activate)
        if getattr(self, '_rec_hidden', False) and self.alive(self.rec_win):
            self.rec_win.deiconify()
        self._rec_hidden = False
        if self.set_win and self.set_win.winfo_exists():
            self.show_win(self.set_win)
        self.ocr.wake.set()

    def _select(self, kind):
        from PIL import Image, ImageEnhance, ImageTk
        try:
            with new_mss() as s:
                mon, prim = s.monitors[0], s.monitors[1]
                shot = s.grab(mon)
            orig = Image.frombytes('RGB', shot.size, shot.bgra, 'raw', 'BGRX')
            img = ImageEnhance.Brightness(orig).enhance(0.6)
            top = tk.Toplevel(self.root)
            top.overrideredirect(True)
            top.attributes('-topmost', True)
            top.geometry(f"{mon['width']}x{mon['height']}+{mon['left']}+{mon['top']}")
            cv = tk.Canvas(top, highlightthickness=0, cursor='crosshair', bg='black')
            cv.pack(fill='both', expand=True)
            top._photo = ImageTk.PhotoImage(img)
            cv.create_image(0, 0, image=top._photo, anchor='nw')
        except Exception:
            self.restore_after_select()
            raise

        px, py = prim['left'] - mon['left'], prim['top'] - mon['top']
        pw, ph = prim['width'], prim['height']
        cv.create_rectangle(px + 3, py + 3, px + pw - 3, py + ph - 3, outline=C['warn'], width=6)
        size = max(14, ph // 90)
        hp, mp = self.cfg['hp_name'], self.cfg['mp_name']
        name, hint = {
            'exp': ('畫面下方的 EXP 數字（含 %）', '只框數字那一行就好'),
            'hp': (f'快捷欄上{hp}的數量', '只框數字那一行就好'),
            'mp': (f'快捷欄上{mp}的數量', '只框數字那一行就好'),
            'inv': ('整個背包的格子區域', '框消耗欄範圍，涵蓋自定義的所有藥水'),
            'hp_tpl': (f'背包裡「一格」{hp}', '從圖示上緣框到數字下緣，左右框滿那一格'),
            'mp_tpl': (f'背包裡「一格」{mp}', '從圖示上緣框到數字下緣，左右框滿那一格'),
            'meso': ('背包下方的楓幣數字', '只框數字那一行就好'),
        }[kind]
        t1 = cv.create_text(px + pw // 2, py + ph // 12, text=f'框選模式：用滑鼠拖曳，框住{name}',
                            fill='#ffffff', font=(FONT, size, 'bold'))
        t2 = cv.create_text(px + pw // 2, py + ph // 12 + int(size * 2.2),
                            text=f'{hint}。按右鍵或 Esc 取消', fill='#ffd8a8', font=(FONT, int(size * 0.7)))
        x0, y0, x1, y1 = cv.bbox(t1)
        _, _, x3, y3 = cv.bbox(t2)
        bg = cv.create_rectangle(min(x0, cv.bbox(t2)[0]) - size, y0 - size // 2, max(x1, x3) + size, y3 + size // 2,
                                 fill='#141922', outline=C['warn'], width=3)
        cv.tag_lower(bg, t1)
        st = {}
        closed = []

        def done():
            if closed:
                return
            closed.append(1)
            top.destroy()
            self.restore_after_select()

        def down(e):
            st['x0'], st['y0'] = e.x, e.y
            st['r'] = cv.create_rectangle(e.x, e.y, e.x, e.y, outline='#ff3b30', width=3)
            st['t'] = cv.create_text(e.x, e.y - 8, text='', anchor='sw', fill='#ff8a80', font=(MONO, max(10, size // 2), 'bold'))

        def move(e):
            if 'r' in st:
                cv.coords(st['r'], st['x0'], st['y0'], e.x, e.y)
                cv.itemconfigure(st['t'], text=f"{abs(e.x - st['x0'])}×{abs(e.y - st['y0'])}")
                cv.coords(st['t'], min(st['x0'], e.x), min(st['y0'], e.y) - 8)

        def up(e):
            if 'r' not in st:
                return
            x, y = min(st['x0'], e.x), min(st['y0'], e.y)
            ww, hh = abs(e.x - st['x0']), abs(e.y - st['y0'])
            if ww < 6 or hh < 6:
                cv.delete(st.pop('r'), st.pop('t'))
                return
            try:
                if kind.endswith('_tpl'):
                    orig.crop((x, y, x + ww, y + hh)).save(tpl_path(kind[:2]))
                    self.inv_msg = ''
                    self.forget_potion(kind[:2])
                else:
                    self.cfg[kind + '_region'] = [mon['left'] + x, mon['top'] + y, ww, hh]
                    self.raw[kind] = ''
                    if kind == 'exp':
                        self.pend_dec, self.reject_n = None, 0
                        self.exp_last = self.need = self.pend_first = self.pre_inc = self.rej_run = None
                    if kind in ('hp', 'mp'):
                        self.qs[kind] = self.qs_pend[kind] = self.cap_since[kind] = None
                    self.save_cfg()
                self.say(f'已框選：{name}', 5)
            except Exception as ex:
                log_error(traceback.format_exc())
                self.say(f'出錯了：存不了框選的結果（{ex}）', 10)
            finally:
                done()

        cv.bind('<ButtonPress-1>', down)
        cv.bind('<B1-Motion>', move)
        cv.bind('<ButtonRelease-1>', up)
        cv.bind('<Button-3>', lambda e: done())
        top.bind('<Escape>', lambda e: done())
        top.after(60, top.focus_force)
        top.after(120000, done)

    def run(self):
        self.root.mainloop()


_INSTANCE = re.sub(r'\W', '', os.environ.get('EXPOVERLAY_INSTANCE', ''))
MUTEX_NAME = 'Local\\ExpOverlayArtale' + _INSTANCE
QUIT_EVENT = 'Local\\ExpOverlayArtale_Quit' + _INSTANCE
kernel32.CreateMutexW.restype = wintypes.HANDLE
kernel32.CreateEventW.restype = wintypes.HANDLE
kernel32.OpenEventW.restype = wintypes.HANDLE
kernel32.SetEvent.argtypes = [wintypes.HANDLE]
kernel32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
kernel32.WaitForSingleObject.restype = wintypes.DWORD
kernel32.ReleaseMutex.argtypes = [wintypes.HANDLE]
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]


def main():
    mutex = kernel32.CreateMutexW(None, True, MUTEX_NAME)
    if kernel32.GetLastError() == 183:
        ans = user32.MessageBoxW(None, '經驗收益計算器已經開著了。\n\n要關掉舊的、換成重新開一個嗎？\n'
                                       '（舊的進度會先存起來，新的會接著算）\n\n'
                                       '按「否」維持原本的。找不到浮窗的話，按 Ctrl+F12 顯示／隱藏。',
                                 '經驗收益計算器', 0x4 | 0x20 | 0x40000)
        if ans != 6:
            kernel32.CloseHandle(mutex)
            return
        if kernel32.WaitForSingleObject(mutex, 0) not in (0, 0x80):
            evt = kernel32.OpenEventW(0x0002, False, QUIT_EVENT)
            if evt:
                kernel32.SetEvent(evt)
                kernel32.CloseHandle(evt)
            if kernel32.WaitForSingleObject(mutex, 10000 if evt else 1500) not in (0, 0x80):
                msg = ('舊的浮窗沒有關掉。\n\n請按它右上角的 ✕，再開一次。' if evt else
                       '舊版的浮窗沒辦法自動關閉。\n\n請按舊浮窗右上角的 ✕ 關掉（會存紀錄），再開一次。')
                user32.MessageBoxW(None, msg, '經驗收益計算器', 0x30 | 0x40000)
                kernel32.CloseHandle(mutex)
                return
    threading.excepthook = lambda a: log_error(''.join(
        traceback.format_exception(a.exc_type, a.exc_value, a.exc_traceback)))
    try:
        App().run()
    except Exception:
        log_error(traceback.format_exc())
        user32.MessageBoxW(None, '經驗收益計算器啟動失敗，詳細內容在 error.log。\n\n' + traceback.format_exc()[-600:],
                           '經驗收益計算器', 0x10 | 0x40000)
    finally:
        kernel32.ReleaseMutex(mutex)
        kernel32.CloseHandle(mutex)


if __name__ == '__main__':
    main()
