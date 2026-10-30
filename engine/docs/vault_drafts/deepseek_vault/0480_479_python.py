if use_poler and poler_evol:
    dm = poler_evol.evolve(dm, ...)  # эволюция вместо диагонализации
else:
    fock = mf.get_fock(...)          # стандартный путь
mo_energy, mo_coeff = mf.eig(fock, s1e)
dm = mf.make_rdm1(mo_coeff, mo_occ)
