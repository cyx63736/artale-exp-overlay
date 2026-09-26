@echo off
cd /d "%~dp0"
py -3.12 -m pip install -r requirements.txt pyinstaller
py -3.12 -m PyInstaller --noconfirm --clean --onedir --windowed --name ExpOverlay --collect-all rapidocr_onnxruntime --collect-binaries onnxruntime --hidden-import mss --hidden-import PIL.ImageTk exp_overlay.py
del /q "dist\ExpOverlay\_internal\cv2\opencv_videoio_ffmpeg*.dll" 2>nul
