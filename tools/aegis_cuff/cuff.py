import json, numpy as np
L = json.load(open('layout.json'))
C = L['cuff']
R_BORE = C['bore_mm']/2; T0 = C['thick_mm']; BAND = C['band_mm']; CREST = C['crest_mm']
CDEG, POW, TIP, RISE = C['crest_deg'], C['crest_pow'], C['tip_deg'], C['crest_rise_mm']
R_REF = R_BORE + T0
PHI_END = 180.0 - np.degrees(np.arcsin(C['gap_mm']/2/R_BORE))
S_END = np.radians(PHI_END)*R_REF
RHO = C['end_corner_mm']
def share(phi_deg):
    soft = np.sqrt(phi_deg*phi_deg + TIP*TIP) - TIP
    return np.clip(1 - soft/CDEG, 0, None)**POW
def hw0(s):
    return 0.5*(BAND + (CREST - BAND)*share(np.degrees(np.asarray(s, float)/R_REF)))
def hw(s):
    s = np.atleast_1d(np.asarray(s, float)); a = np.abs(s); out = np.array(hw0(s), float)
    k = a > S_END - RHO
    x = a[k] - (S_END - RHO)
    out[k] = out[k] - RHO + np.sqrt(np.maximum(RHO*RHO - x*x, 0))
    return out
def thick(s):
    return T0 + RISE*share(np.degrees(np.asarray(s, float)/R_REF))
