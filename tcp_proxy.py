import socket, threading
def proxy(src, dst):
    try:
        while d := src.recv(65536):
            dst.sendall(d)
    except: pass
    src.close(); dst.close()
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(('0.0.0.0', 8001)); s.listen(5)
print('Proxy listening on :8001', flush=True)
while True:
    c, _ = s.accept()
    r = socket.create_connection(('ayourtch-desktop', 8000))
    threading.Thread(target=proxy, args=(c,r), daemon=True).start()
    threading.Thread(target=proxy, args=(r,c), daemon=True).start()