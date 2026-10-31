import sys
trits = [int(t) for t in sys.argv[1].split(',')]
name = sys.argv[2] if len(sys.argv) > 2 else 'D'
def d3(t):
    e  = {1: 'om', 0: '1', -1: 'om^2'}[t]   # om^t
    e2 = {1: 'om^2', 0: '1', -1: 'om^4'}[t] # om^{2t}
    return f"[1,0,0; 0,{e},0; 0,0,{e2}]"
mats = [d3(t) for t in trits]
D = mats[0]
for m in mats[1:]:
    D = f"kron({D}, {m})"
print(f"calc let {name} = {D}")
dec = [(-t) % 3 for t in trits]
vecs = [f"[{1 if c==0 else 0};{1 if c==1 else 0};{1 if c==2 else 0}]" for c in dec]
SEL = vecs[0]
for v in vecs[1:]:
    SEL = f"kron({SEL}, {v})"
print(f"calc let SEL{name} = {SEL}")
