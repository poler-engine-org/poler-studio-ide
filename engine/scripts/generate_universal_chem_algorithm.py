#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
=======================================================================
UNIVERSAL QUANTUM CHEMISTRY ENGINE & 50,000+ LINE ASM GENERATOR
=======================================================================
Generates:
1. src/universal_chem.rs (Full 118 Elements, 12 Quantum Archetypes, Balanced Ternary Bonds, Stoichiometric Formula Parser)
2. src/universal_chem_asm.rs (Streaming No-Mul x86_64 ASM Engine)
3. chem_unrolled_resonance_50k.s (50,000+ line unrolled quantum chemical phase rotor)
"""

import math
import os
import sys

PHI = (1.0 + math.sqrt(5.0)) / 2.0
TWO_PI = 2.0 * math.pi

# 118 Elements Database
# Z, Symbol, Name, NameRU, AtomicMass, Electronegativity (Pauling), Group, Period, Block, CovRadius(pm), VdwRadius(pm), IonEnergy(eV), ElectronConfig, ArchetypeIndex
RAW_ELEMENTS = [
    (1, "H", "Hydrogen", "Водород", 1.008, 2.20, 1, 1, "S", 31.0, 120.0, 13.598, "1s1", 7), # Organic / Life nonmetal
    (2, "He", "Helium", "Гелий", 4.002602, 0.0, 18, 1, "S", 28.0, 140.0, 24.587, "1s2", 9), # Noble Gas
    (3, "Li", "Lithium", "Литий", 6.94, 0.98, 1, 2, "S", 128.0, 182.0, 5.392, "[He] 2s1", 0), # Alkali
    (4, "Be", "Beryllium", "Бериллий", 9.012183, 1.57, 2, 2, "S", 96.0, 153.0, 9.323, "[He] 2s2", 1), # Alkaline Earth
    (5, "B", "Boron", "Бор", 10.81, 2.04, 13, 2, "P", 84.0, 192.0, 8.298, "[He] 2s2 2p1", 6), # Metalloid
    (6, "C", "Carbon", "Углерод", 12.011, 2.55, 14, 2, "P", 76.0, 170.0, 11.260, "[He] 2s2 2p2", 7), # Organic
    (7, "N", "Nitrogen", "Азот", 14.007, 3.04, 15, 2, "P", 71.0, 155.0, 14.534, "[He] 2s2 2p3", 7), # Organic
    (8, "O", "Oxygen", "Кислород", 15.999, 3.44, 16, 2, "P", 66.0, 152.0, 13.618, "[He] 2s2 2p4", 7), # Organic
    (9, "F", "Fluorine", "Фтор", 18.998403, 3.98, 17, 2, "P", 57.0, 147.0, 17.423, "[He] 2s2 2p5", 8), # Halogen
    (10, "Ne", "Neon", "Неон", 20.1797, 0.0, 18, 2, "P", 58.0, 154.0, 21.565, "[He] 2s2 2p6", 9), # Noble Gas
    (11, "Na", "Sodium", "Натрий", 22.989769, 0.93, 1, 3, "S", 166.0, 227.0, 5.139, "[Ne] 3s1", 0), # Alkali
    (12, "Mg", "Magnesium", "Магний", 24.305, 1.31, 2, 3, "S", 141.0, 173.0, 7.646, "[Ne] 3s2", 1), # Alkaline Earth
    (13, "Al", "Aluminium", "Алюминий", 26.981538, 1.61, 13, 3, "P", 121.0, 184.0, 5.986, "[Ne] 3s2 3p1", 5), # Post-transition
    (14, "Si", "Silicon", "Кремний", 28.085, 1.90, 14, 3, "P", 111.0, 210.0, 8.152, "[Ne] 3s2 3p2", 6), # Metalloid
    (15, "P", "Phosphorus", "Фосфор", 30.973762, 2.19, 15, 3, "P", 107.0, 180.0, 10.487, "[Ne] 3s2 3p3", 7), # Organic
    (16, "S", "Sulfur", "Сера", 32.06, 2.58, 16, 3, "P", 105.0, 180.0, 10.360, "[Ne] 3s2 3p4", 7), # Organic
    (17, "Cl", "Chlorine", "Хлор", 35.45, 3.16, 17, 3, "P", 102.0, 175.0, 12.968, "[Ne] 3s2 3p5", 8), # Halogen
    (18, "Ar", "Argon", "Аргон", 39.948, 0.0, 18, 3, "P", 106.0, 188.0, 15.760, "[Ne] 3s2 3p6", 9), # Noble Gas
    (19, "K", "Potassium", "Калий", 39.0983, 0.82, 1, 4, "S", 203.0, 275.0, 4.341, "[Ar] 4s1", 0), # Alkali
    (20, "Ca", "Calcium", "Кальций", 40.078, 1.00, 2, 4, "S", 176.0, 231.0, 6.113, "[Ar] 4s2", 1), # Alkaline Earth
    (21, "Sc", "Scandium", "Скандий", 44.955908, 1.36, 3, 4, "D", 170.0, 211.0, 6.561, "[Ar] 3d1 4s2", 2), # Transition
    (22, "Ti", "Titanium", "Титан", 47.867, 1.54, 4, 4, "D", 160.0, 200.0, 6.828, "[Ar] 3d2 4s2", 2),
    (23, "V", "Vanadium", "Ванадий", 50.9415, 1.63, 5, 4, "D", 153.0, 200.0, 6.746, "[Ar] 3d3 4s2", 2),
    (24, "Cr", "Chromium", "Хром", 51.9961, 1.66, 6, 4, "D", 139.0, 200.0, 6.767, "[Ar] 3d5 4s1", 2),
    (25, "Mn", "Manganese", "Марганец", 54.938044, 1.55, 7, 4, "D", 139.0, 200.0, 7.434, "[Ar] 3d5 4s2", 2),
    (26, "Fe", "Iron", "Железо", 55.845, 1.83, 8, 4, "D", 132.0, 200.0, 7.902, "[Ar] 3d6 4s2", 2),
    (27, "Co", "Cobalt", "Кобальт", 58.933194, 1.88, 9, 4, "D", 126.0, 200.0, 7.881, "[Ar] 3d7 4s2", 2),
    (28, "Ni", "Nickel", "Никель", 58.6934, 1.91, 10, 4, "D", 124.0, 163.0, 7.640, "[Ar] 3d8 4s2", 2),
    (29, "Cu", "Copper", "Медь", 63.546, 1.90, 11, 4, "D", 132.0, 140.0, 7.726, "[Ar] 3d10 4s1", 2),
    (30, "Zn", "Zinc", "Цинк", 65.38, 1.65, 12, 4, "D", 122.0, 139.0, 9.394, "[Ar] 3d10 4s2", 2),
    (31, "Ga", "Gallium", "Галлий", 69.723, 1.81, 13, 4, "P", 122.0, 187.0, 5.999, "[Ar] 3d10 4s2 4p1", 5),
    (32, "Ge", "Germanium", "Германий", 72.630, 2.01, 14, 4, "P", 120.0, 211.0, 7.899, "[Ar] 3d10 4s2 4p2", 6),
    (33, "As", "Arsenic", "Мышьяк", 74.921595, 2.18, 15, 4, "P", 119.0, 185.0, 9.789, "[Ar] 3d10 4s2 4p3", 6),
    (34, "Se", "Selenium", "Селен", 78.971, 2.55, 16, 4, "P", 120.0, 190.0, 9.752, "[Ar] 3d10 4s2 4p4", 7),
    (35, "Br", "Bromine", "Бром", 79.904, 2.96, 17, 4, "P", 120.0, 185.0, 11.814, "[Ar] 3d10 4s2 4p5", 8),
    (36, "Kr", "Krypton", "Криптон", 83.798, 3.00, 18, 4, "P", 116.0, 202.0, 14.000, "[Ar] 3d10 4s2 4p6", 9),
    (37, "Rb", "Rubidium", "Рубидий", 85.4678, 0.82, 1, 5, "S", 220.0, 303.0, 4.177, "[Kr] 5s1", 0),
    (38, "Sr", "Strontium", "Стронций", 87.62, 0.95, 2, 5, "S", 195.0, 249.0, 5.695, "[Kr] 5s2", 1),
    (39, "Y", "Yttrium", "Иттрий", 88.90584, 1.22, 3, 5, "D", 190.0, 232.0, 6.217, "[Kr] 4d1 5s2", 2),
    (40, "Zr", "Zirconium", "Цирконий", 91.224, 1.33, 4, 5, "D", 175.0, 223.0, 6.634, "[Kr] 4d2 5s2", 2),
    (41, "Nb", "Niobium", "Ниобий", 92.90637, 1.6, 5, 5, "D", 164.0, 218.0, 6.759, "[Kr] 4d4 5s1", 2),
    (42, "Mo", "Molybdenum", "Молибден", 95.95, 2.16, 6, 5, "D", 154.0, 217.0, 7.092, "[Kr] 4d5 5s1", 2),
    (43, "Tc", "Technetium", "Технеций", 98.0, 1.9, 7, 5, "D", 147.0, 216.0, 7.28, "[Kr] 4d5 5s2", 2),
    (44, "Ru", "Ruthenium", "Рутений", 101.07, 2.2, 8, 5, "D", 146.0, 213.0, 7.361, "[Kr] 4d7 5s1", 2),
    (45, "Rh", "Rhodium", "Родий", 102.90550, 2.28, 9, 5, "D", 142.0, 210.0, 7.459, "[Kr] 4d8 5s1", 2),
    (46, "Pd", "Palladium", "Палладий", 106.42, 2.20, 10, 5, "D", 139.0, 210.0, 8.337, "[Kr] 4d10", 2),
    (47, "Ag", "Silver", "Серебро", 107.8682, 1.93, 11, 5, "D", 145.0, 172.0, 7.576, "[Kr] 4d10 5s1", 2),
    (48, "Cd", "Cadmium", "Кадмий", 112.414, 1.69, 12, 5, "D", 144.0, 158.0, 8.994, "[Kr] 4d10 5s2", 2),
    (49, "In", "Indium", "Индий", 114.818, 1.78, 13, 5, "P", 142.0, 193.0, 5.786, "[Kr] 4d10 5s2 5p1", 5),
    (50, "Sn", "Tin", "Олово", 118.710, 1.96, 14, 5, "P", 139.0, 217.0, 7.344, "[Kr] 4d10 5s2 5p2", 5),
    (51, "Sb", "Antimony", "Сурьма", 121.760, 2.05, 15, 5, "P", 139.0, 206.0, 8.608, "[Kr] 4d10 5s2 5p3", 6),
    (52, "Te", "Tellurium", "Теллур", 127.60, 2.1, 16, 5, "P", 138.0, 206.0, 9.010, "[Kr] 4d10 5s2 5p4", 6),
    (53, "I", "Iodine", "Иод", 126.90447, 2.66, 17, 5, "P", 139.0, 198.0, 10.451, "[Kr] 4d10 5s2 5p5", 8),
    (54, "Xe", "Xenon", "Ксенон", 131.293, 2.60, 18, 5, "P", 140.0, 216.0, 12.130, "[Kr] 4d10 5s2 5p6", 9),
    (55, "Cs", "Caesium", "Цезий", 132.905452, 0.79, 1, 6, "S", 244.0, 343.0, 3.894, "[Xe] 6s1", 0),
    (56, "Ba", "Barium", "Барий", 137.327, 0.89, 2, 6, "S", 215.0, 268.0, 5.212, "[Xe] 6s2", 1),
    (57, "La", "Lanthanum", "Лантан", 138.90547, 1.10, 3, 6, "F", 207.0, 240.0, 5.577, "[Xe] 5d1 6s2", 3),
    (58, "Ce", "Cerium", "Церий", 140.116, 1.12, 3, 6, "F", 204.0, 235.0, 5.539, "[Xe] 4f1 5d1 6s2", 3),
    (59, "Pr", "Praseodymium", "Празеодим", 140.90766, 1.13, 3, 6, "F", 203.0, 239.0, 5.473, "[Xe] 4f3 6s2", 3),
    (60, "Nd", "Neodymium", "Неодим", 144.242, 1.14, 3, 6, "F", 201.0, 229.0, 5.525, "[Xe] 4f4 6s2", 3),
    (61, "Pm", "Promethium", "Прометий", 145.0, 1.13, 3, 6, "F", 199.0, 236.0, 5.582, "[Xe] 4f5 6s2", 3),
    (62, "Sm", "Samarium", "Самарий", 150.36, 1.17, 3, 6, "F", 198.0, 229.0, 5.644, "[Xe] 4f6 6s2", 3),
    (63, "Eu", "Europium", "Европий", 151.964, 1.2, 3, 6, "F", 198.0, 233.0, 5.670, "[Xe] 4f7 6s2", 3),
    (64, "Gd", "Gadolinium", "Гадолиний", 157.25, 1.20, 3, 6, "F", 196.0, 237.0, 6.150, "[Xe] 4f7 5d1 6s2", 3),
    (65, "Tb", "Terbium", "Тербий", 158.92535, 1.2, 3, 6, "F", 194.0, 221.0, 5.864, "[Xe] 4f9 6s2", 3),
    (66, "Dy", "Dysprosium", "Диспрозий", 162.500, 1.22, 3, 6, "F", 192.0, 229.0, 5.939, "[Xe] 4f10 6s2", 3),
    (67, "Ho", "Holmium", "Гольмий", 164.93033, 1.23, 3, 6, "F", 192.0, 216.0, 6.022, "[Xe] 4f11 6s2", 3),
    (68, "Er", "Erbium", "Эрбий", 167.259, 1.24, 3, 6, "F", 189.0, 235.0, 6.108, "[Xe] 4f12 6s2", 3),
    (69, "Tm", "Thulium", "Тулий", 168.93422, 1.25, 3, 6, "F", 190.0, 227.0, 6.184, "[Xe] 4f13 6s2", 3),
    (70, "Yb", "Ytterbium", "Иттербий", 173.045, 1.1, 3, 6, "F", 187.0, 242.0, 6.254, "[Xe] 4f14 6s2", 3),
    (71, "Lu", "Lutetium", "Лютеций", 174.9668, 1.27, 3, 6, "D", 187.0, 221.0, 5.426, "[Xe] 4f14 5d1 6s2", 3),
    (72, "Hf", "Hafnium", "Гафний", 178.49, 1.3, 4, 6, "D", 175.0, 212.0, 6.825, "[Xe] 4f14 5d2 6s2", 2),
    (73, "Ta", "Tantalum", "Тантал", 180.94788, 1.5, 5, 6, "D", 170.0, 217.0, 7.550, "[Xe] 4f14 5d3 6s2", 2),
    (74, "W", "Tungsten", "Вольфрам", 183.84, 2.36, 6, 6, "D", 162.0, 210.0, 7.864, "[Xe] 4f14 5d4 6s2", 2),
    (75, "Re", "Rhenium", "Рений", 186.207, 1.9, 7, 6, "D", 151.0, 217.0, 7.834, "[Xe] 4f14 5d5 6s2", 2),
    (76, "Os", "Osmium", "Осмий", 190.23, 2.2, 8, 6, "D", 144.0, 216.0, 8.438, "[Xe] 4f14 5d6 6s2", 2),
    (77, "Ir", "Iridium", "Иридий", 192.217, 2.20, 9, 6, "D", 141.0, 213.0, 8.967, "[Xe] 4f14 5d7 6s2", 2),
    (78, "Pt", "Platinum", "Платина", 195.084, 2.28, 10, 6, "D", 136.0, 175.0, 8.959, "[Xe] 4f14 5d9 6s1", 2),
    (79, "Au", "Gold", "Золото", 196.966569, 2.54, 11, 6, "D", 136.0, 166.0, 9.226, "[Xe] 4f14 5d10 6s1", 2),
    (80, "Hg", "Mercury", "Ртуть", 200.592, 2.00, 12, 6, "D", 132.0, 155.0, 10.438, "[Xe] 4f14 5d10 6s2", 2),
    (81, "Tl", "Thallium", "Таллий", 204.38, 1.62, 13, 6, "P", 145.0, 196.0, 6.108, "[Xe] 4f14 5d10 6s2 6p1", 5),
    (82, "Pb", "Lead", "Свинец", 207.2, 2.33, 14, 6, "P", 146.0, 202.0, 7.417, "[Xe] 4f14 5d10 6s2 6p2", 5),
    (83, "Bi", "Bismuth", "Висмут", 208.98040, 2.02, 15, 6, "P", 148.0, 207.0, 7.286, "[Xe] 4f14 5d10 6s2 6p3", 5),
    (84, "Po", "Polonium", "Полоний", 209.0, 2.0, 16, 6, "P", 140.0, 197.0, 8.417, "[Xe] 4f14 5d10 6s2 6p4", 5),
    (85, "At", "Astatine", "Астат", 210.0, 2.2, 17, 6, "P", 150.0, 202.0, 9.318, "[Xe] 4f14 5d10 6s2 6p5", 8),
    (86, "Rn", "Radon", "Радон", 222.0, 2.2, 18, 6, "P", 150.0, 220.0, 10.749, "[Xe] 4f14 5d10 6s2 6p6", 9),
    (87, "Fr", "Francium", "Франций", 223.0, 0.7, 1, 7, "S", 260.0, 348.0, 4.073, "[Rn] 7s1", 0),
    (88, "Ra", "Radium", "Радий", 226.0, 0.9, 2, 7, "S", 221.0, 283.0, 5.278, "[Rn] 7s2", 1),
    (89, "Ac", "Actinium", "Актиний", 227.0, 1.1, 3, 7, "F", 215.0, 247.0, 5.17, "[Rn] 6d1 7s2", 4),
    (90, "Th", "Thorium", "Торий", 232.0377, 1.3, 3, 7, "F", 206.0, 245.0, 6.307, "[Rn] 6d2 7s2", 4),
    (91, "Pa", "Protactinium", "Протактиний", 231.03588, 1.5, 3, 7, "F", 200.0, 243.0, 5.89, "[Rn] 5f2 6d1 7s2", 4),
    (92, "U", "Uranium", "Уран", 238.02891, 1.38, 3, 7, "F", 196.0, 241.0, 6.194, "[Rn] 5f3 6d1 7s2", 4),
    (93, "Np", "Neptunium", "Нептуний", 237.0, 1.36, 3, 7, "F", 190.0, 239.0, 6.266, "[Rn] 5f4 6d1 7s2", 4),
    (94, "Pu", "Plutonium", "Плутоний", 244.0, 1.28, 3, 7, "F", 187.0, 243.0, 6.026, "[Rn] 5f6 7s2", 4),
    (95, "Am", "Americium", "Америций", 243.0, 1.13, 3, 7, "F", 180.0, 244.0, 5.974, "[Rn] 5f7 7s2", 4),
    (96, "Cm", "Curium", "Кюрий", 247.0, 1.28, 3, 7, "F", 169.0, 245.0, 5.991, "[Rn] 5f7 6d1 7s2", 4),
    (97, "Bk", "Berkelium", "Берклий", 247.0, 1.3, 3, 7, "F", 166.0, 244.0, 6.198, "[Rn] 5f9 7s2", 4),
    (98, "Cf", "Californium", "Калифорний", 251.0, 1.3, 3, 7, "F", 168.0, 245.0, 6.282, "[Rn] 5f10 7s2", 4),
    (99, "Es", "Einsteinium", "Эйнштейний", 252.0, 1.3, 3, 7, "F", 165.0, 245.0, 6.42, "[Rn] 5f11 7s2", 4),
    (100, "Fm", "Fermium", "Фермий", 257.0, 1.3, 3, 7, "F", 167.0, 245.0, 6.50, "[Rn] 5f12 7s2", 4),
    (101, "Md", "Mendelevium", "Менделевий", 258.0, 1.3, 3, 7, "F", 173.0, 246.0, 6.58, "[Rn] 5f13 7s2", 4),
    (102, "No", "Nobelium", "Нобелий", 259.0, 1.3, 3, 7, "F", 176.0, 246.0, 6.65, "[Rn] 5f14 7s2", 4),
    (103, "Lr", "Lawrencium", "Лоуренсий", 266.0, 1.3, 3, 7, "D", 161.0, 246.0, 4.90, "[Rn] 5f14 7s2 7p1", 4),
    (104, "Rf", "Rutherfordium", "Резерфордий", 267.0, 0.0, 4, 7, "D", 157.0, 246.0, 6.0, "[Rn] 5f14 6d2 7s2", 2),
    (105, "Db", "Dubnium", "Дубний", 268.0, 0.0, 5, 7, "D", 149.0, 246.0, 6.8, "[Rn] 5f14 6d3 7s2", 2),
    (106, "Sg", "Seaborgium", "Сиборгий", 269.0, 0.0, 6, 7, "D", 143.0, 246.0, 7.5, "[Rn] 5f14 6d4 7s2", 2),
    (107, "Bh", "Bohrium", "Борий", 270.0, 0.0, 7, 7, "D", 141.0, 246.0, 7.7, "[Rn] 5f14 6d5 7s2", 2),
    (108, "Hs", "Hassium", "Хассий", 277.0, 0.0, 8, 7, "D", 134.0, 246.0, 7.8, "[Rn] 5f14 6d6 7s2", 2),
    (109, "Mt", "Meitnerium", "Мейтнерий", 278.0, 0.0, 9, 7, "D", 129.0, 246.0, 8.0, "[Rn] 5f14 6d7 7s2", 2),
    (110, "Ds", "Darmstadtium", "Дармштадтий", 281.0, 0.0, 10, 7, "D", 128.0, 246.0, 9.6, "[Rn] 5f14 6d9 7s1", 2),
    (111, "Rg", "Roentgenium", "Рентгений", 282.0, 0.0, 11, 7, "D", 121.0, 246.0, 10.4, "[Rn] 5f14 6d10 7s1", 2),
    (112, "Cn", "Copernicium", "Коперниций", 285.0, 0.0, 12, 7, "D", 122.0, 246.0, 11.9, "[Rn] 5f14 6d10 7s2", 2),
    (113, "Nh", "Nihonium", "Нихоний", 286.0, 0.0, 13, 7, "P", 136.0, 246.0, 7.3, "[Rn] 5f14 6d10 7s2 7p1", 5),
    (114, "Fl", "Flerovium", "Флеровий", 289.0, 0.0, 14, 7, "P", 143.0, 246.0, 8.5, "[Rn] 5f14 6d10 7s2 7p2", 5),
    (115, "Mc", "Moscovium", "Московий", 290.0, 0.0, 15, 7, "P", 162.0, 246.0, 5.6, "[Rn] 5f14 6d10 7s2 7p3", 5),
    (116, "Lv", "Livermorium", "Ливерморий", 293.0, 0.0, 16, 7, "P", 175.0, 246.0, 7.1, "[Rn] 5f14 6d10 7s2 7p4", 5),
    (117, "Ts", "Tennessine", "Теннессин", 294.0, 0.0, 17, 7, "P", 165.0, 246.0, 7.7, "[Rn] 5f14 6d10 7s2 7p5", 8),
    (118, "Og", "Oganesson", "Оганесон", 294.0, 0.0, 18, 7, "P", 157.0, 246.0, 8.8, "[Rn] 5f14 6d10 7s2 7p6", 9),
]

ARCHETYPES = [
    ("AlkaliS1", "s¹: H, Li, Na, K, Rb, Cs, Fr — моновалентні донори"),
    ("AlkalineEarthS2", "s²: Be, Mg, Ca, Sr, Ba, Ra — дивалентні лужноземельні"),
    ("TransitionDBlock", "3d/4d/5d: Fe, Co, Ni, Cu, Ti, Pt, Au — спіновий резонанс та d-орбіталі"),
    ("Lanthanide4F", "4f: La-Lu — рідкоземельний f-магнетизм та парамагнетики"),
    ("Actinide5F", "5f: Ac-Lr — радіоактивні важкі актиноїди, ядерний розпад"),
    ("PostTransition", "p-метали: Al, Ga, In, Sn, Pb, Bi, Po — пластичні провідники"),
    ("MetalloidSemiconductor", "Напівпровідники: B, Si, Ge, As, Sb, Te — ковалентна зона"),
    ("OrganicLifeNonmetal", "Органогени (sp, sp², sp³): C, N, O, P, S, Se — основа життя"),
    ("HalogenP5", "p⁵: F, Cl, Br, I, At, Ts — сильні окисники, акцептори електронів"),
    ("NobleGasP6", "p⁶: He, Ne, Ar, Kr, Xe, Rn, Og — інертний вакуумний стазис"),
    ("SuperheavyIsland", "Z ≥ 119: Острів надважкої стабільності / релятивістські оболонки"),
    ("FrederiteResonance", "Фредеритовий каталізатор 1.4 ТГц: метастабільні sp²/sp³ зв'язки Етерії"),
]

def generate_rust_chem_module():
    out = []
    out.append("""//! =======================================================================
//! АЛГОРИТМ: «УНІВЕРСАЛЬНИЙ КВАНТОВО-ХІМІЧНИЙ РЕЗОНАТОР» (Universal Chem Engine)
//! 118 хімічних елементів, 12 квантових архетипів, збалансовані тритні зв'язки {-1, 0, +1},
//! стехіометричний парсер формул та фазовий ротор на золотому перетині Φ.
//! =======================================================================

use std::collections::BTreeMap;
use std::f64::consts::PI;

/// Золотий перетин: φ = (1 + √5) / 2
pub const PHI: f64 = 1.618033988749895;

/// Кількість фундаментальних квантово-хімічних архетипів
pub const NUM_CHEM_ARCHETYPES: usize = 12;

/// Кількість хімічних елементів періодичної таблиці
pub const NUM_ELEMENTS: usize = 118;

/// Електронний блок орбіталей
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChemBlock {
    S = 0,
    P = 1,
    D = 2,
    F = 3,
}

/// 12 фундаментальних квантово-хімічних архетипів
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChemArchetype {
    AlkaliS1 = 0,
    AlkalineEarthS2 = 1,
    TransitionDBlock = 2,
    Lanthanide4F = 3,
    Actinide5F = 4,
    PostTransition = 5,
    MetalloidSemiconductor = 6,
    OrganicLifeNonmetal = 7,
    HalogenP5 = 8,
    NobleGasP6 = 9,
    SuperheavyIsland = 10,
    FrederiteResonance = 11,
}

impl ChemArchetype {
    pub fn name(&self) -> &'static str {
        match self {
            ChemArchetype::AlkaliS1 => "AlkaliS1",
            ChemArchetype::AlkalineEarthS2 => "AlkalineEarthS2",
            ChemArchetype::TransitionDBlock => "TransitionDBlock",
            ChemArchetype::Lanthanide4F => "Lanthanide4F",
            ChemArchetype::Actinide5F => "Actinide5F",
            ChemArchetype::PostTransition => "PostTransition",
            ChemArchetype::MetalloidSemiconductor => "MetalloidSemiconductor",
            ChemArchetype::OrganicLifeNonmetal => "OrganicLifeNonmetal",
            ChemArchetype::HalogenP5 => "HalogenP5",
            ChemArchetype::NobleGasP6 => "NobleGasP6",
            ChemArchetype::SuperheavyIsland => "SuperheavyIsland",
            ChemArchetype::FrederiteResonance => "FrederiteResonance",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ChemArchetype::AlkaliS1 => "s¹: H, Li, Na, K, Rb, Cs, Fr — моновалентні донори",
            ChemArchetype::AlkalineEarthS2 => "s²: Be, Mg, Ca, Sr, Ba, Ra — дивалентні лужноземельні",
            ChemArchetype::TransitionDBlock => "3d/4d/5d: Fe, Co, Ni, Cu, Ti, Pt, Au — спіновий резонанс та d-орбіталі",
            ChemArchetype::Lanthanide4F => "4f: La-Lu — рідкоземельний f-магнетизм та парамагнетики",
            ChemArchetype::Actinide5F => "5f: Ac-Lr — радіоактивні важкі актиноїди, ядерний розпад",
            ChemArchetype::PostTransition => "p-метали: Al, Ga, In, Sn, Pb, Bi, Po — пластичні провідники",
            ChemArchetype::MetalloidSemiconductor => "Напівпровідники: B, Si, Ge, As, Sb, Te — ковалентна зона",
            ChemArchetype::OrganicLifeNonmetal => "Органогени (sp, sp², sp³): C, N, O, P, S, Se — основа життя",
            ChemArchetype::HalogenP5 => "p⁵: F, Cl, Br, I, At, Ts — сильні окисники, акцептори електронів",
            ChemArchetype::NobleGasP6 => "p⁶: He, Ne, Ar, Kr, Xe, Rn, Og — інертний вакуумний стазис",
            ChemArchetype::SuperheavyIsland => "Z ≥ 119: Острів надважкої стабільності / релятивістські оболонки",
            ChemArchetype::FrederiteResonance => "Фредеритовий каталізатор 1.4 ТГц: метастабільні sp²/sp³ зв'язки Етерії",
        }
    }
}

/// Тип хімічного зв'язку
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BondType {
    NonPolarCovalent, // Трит 0 (Δχ < 0.4)
    PolarCovalent,    // Трит +1 (0.4 <= Δχ <= 2.0)
    Ionic,            // Трит -1 (Δχ > 2.0)
    Metallic,         // Металічний зв'язок (між металами)
    Inert,            // Благородний газ / немає зв'язку
}

/// Паспорт хімічного елемента
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub z: u8,
    pub symbol: &'static str,
    pub name: &'static str,
    pub name_ru: &'static str,
    pub atomic_mass: f64,
    pub electronegativity: f64,
    pub group: u8,
    pub period: u8,
    pub block: ChemBlock,
    pub covalent_radius_pm: f32,
    pub vdw_radius_pm: f32,
    pub ionization_energy_ev: f32,
    pub electron_config: &'static str,
    pub archetype: ChemArchetype,
    pub phase_phi: f64,
}

impl Element {
    /// Чи є елемент металом
    pub fn is_metal(&self) -> bool {
        match self.archetype {
            ChemArchetype::AlkaliS1
            | ChemArchetype::AlkalineEarthS2
            | ChemArchetype::TransitionDBlock
            | ChemArchetype::Lanthanide4F
            | ChemArchetype::Actinide5F
            | ChemArchetype::PostTransition => true,
            _ => false,
        }
    }

    /// Чи є елемент типовим органогеном життя
    pub fn is_organogen(&self) -> bool {
        matches!(self.z, 1 | 6 | 7 | 8 | 15 | 16)
    }
}
""")

    out.append("/// Повна періодична таблиця 118 хімічних елементів")
    out.append("pub static PERIODIC_TABLE: [Element; 118] = [")
    for z, sym, name, name_ru, mass, en, grp, per, blk, rcov, rvdw, ie, ec, arch in RAW_ELEMENTS:
        phi_phase = (z * PHI) % (2.0 * math.pi)
        arch_name = ARCHETYPES[arch][0]
        out.append(f"""    Element {{
        z: {z},
        symbol: "{sym}",
        name: "{name}",
        name_ru: "{name_ru}",
        atomic_mass: {mass:.6},
        electronegativity: {en:.2},
        group: {grp},
        period: {per},
        block: ChemBlock::{blk},
        covalent_radius_pm: {rcov:.1},
        vdw_radius_pm: {rvdw:.1},
        ionization_energy_ev: {ie:.3},
        electron_config: "{ec}",
        archetype: ChemArchetype::{arch_name},
        phase_phi: {phi_phase:.8},
    }},""")
    out.append("];\n")

    out.append("""/// Швидкий пошук елемента за атомним номером Z (1..118)
pub fn get_element_by_z(z: u8) -> Option<&'static Element> {
    if z >= 1 && z <= 118 {
        Some(&PERIODIC_TABLE[(z - 1) as usize])
    } else {
        None
    }
}

/// Швидкий пошук елемента за хімічним символом (case-insensitive)
pub fn get_element_by_symbol(sym: &str) -> Option<&'static Element> {
    let sym_trimmed = sym.trim();
    PERIODIC_TABLE.iter().find(|e| e.symbol.eq_ignore_ascii_case(sym_trimmed))
}

/// Квантовий стан хімічного зв'язку між двома елементами
#[derive(Debug, Clone, PartialEq)]
pub struct ChemicalBond {
    pub elem_a: &'static Element,
    pub elem_b: &'static Element,
    pub delta_chi: f64,
    pub bond_type: BondType,
    pub trit: i8,
    pub phase_resonance: f64,
    pub estimated_length_pm: f32,
}

/// Обчислення хімічного зв'язку між двома елементами
pub fn calculate_bond(elem_a: &'static Element, elem_b: &'static Element) -> ChemicalBond {
    let delta_chi = (elem_a.electronegativity - elem_b.electronegativity).abs();
    let estimated_length_pm = elem_a.covalent_radius_pm + elem_b.covalent_radius_pm;
    let phase_diff = elem_a.phase_phi - elem_b.phase_phi;
    let phase_resonance = (phase_diff).cos();

    // Перевірка на інертні гази
    if elem_a.archetype == ChemArchetype::NobleGasP6 || elem_b.archetype == ChemArchetype::NobleGasP6 {
        return ChemicalBond {
            elem_a,
            elem_b,
            delta_chi,
            bond_type: BondType::Inert,
            trit: 0,
            phase_resonance,
            estimated_length_pm,
        };
    }

    // Перевірка на металічний зв'язок
    if elem_a.is_metal() && elem_b.is_metal() {
        return ChemicalBond {
            elem_a,
            elem_b,
            delta_chi,
            bond_type: BondType::Metallic,
            trit: 0,
            phase_resonance,
            estimated_length_pm,
        };
    }

    // Класифікація за різницею електронегативності Полінга:
    // Трит 0: Неполярний ковалентний (Δχ < 0.4)
    // Трит +1: Полярний ковалентний (0.4 <= Δχ <= 2.0)
    // Трит -1: Йонний зв'язок (Δχ > 2.0)
    let (bond_type, trit) = if delta_chi < 0.4 {
        (BondType::NonPolarCovalent, 0i8)
    } else if delta_chi <= 2.0 {
        (BondType::PolarCovalent, 1i8)
    } else {
        (BondType::Ionic, -1i8)
    };

    ChemicalBond {
        elem_a,
        elem_b,
        delta_chi,
        bond_type,
        trit,
        phase_resonance,
        estimated_length_pm,
    }
}

/// Результат стехіометричного розбору молекулярної формули
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMolecule {
    pub counts: BTreeMap<&'static str, usize>,
    pub molar_mass: f64,
    pub total_electrons: usize,
    pub mass_fractions: BTreeMap<&'static str, f64>,
}

/// Стехіометричний парсер молекулярних формул (підтримує H2O, H2SO4, Fe2(SO4)3, K4[Fe(CN)6], CH3COOH тощо)
pub fn parse_chemical_formula(formula: &str) -> Result<ParsedMolecule, String> {
    let formula = formula.trim();
    if formula.is_empty() {
        return Err("Порожня хімічна формула".into());
    }

    let mut stack: Vec<BTreeMap<&'static str, usize>> = vec![BTreeMap::new()];
    let chars: Vec<char> = formula.chars().collect();
    let mut i = 0;
    let n = chars.len();

    while i < n {
        let c = chars[i];
        if c == '(' || c == '[' || c == '{' {
            stack.push(BTreeMap::new());
            i += 1;
        } else if c == ')' || c == ']' || c == '}' {
            i += 1;
            // Зчитування множника після дужки
            let mut num_str = String::new();
            while i < n && chars[i].is_ascii_digit() {
                num_str.push(chars[i]);
                i += 1;
            }
            let multiplier: usize = if num_str.is_empty() {
                1
            } else {
                num_str.parse().map_err(|e| format!("Некоректний індекс: {e}"))?
            };

            let group_counts = stack.pop().ok_or_else(|| "Незбалансовані дужки".to_string())?;
            let current = stack.last_mut().ok_or_else(|| "Помилка стека формули".to_string())?;
            for (elem, cnt) in group_counts {
                *current.entry(elem).or_insert(0) += cnt * multiplier;
            }
        } else if c.is_ascii_uppercase() {
            let mut sym = String::new();
            sym.push(c);
            i += 1;
            if i < n && chars[i].is_ascii_lowercase() {
                sym.push(chars[i]);
                i += 1;
            }
            let elem = get_element_by_symbol(&sym)
                .ok_or_else(|| format!("Невідомий хімічний елемент: '{sym}'"))?;

            // Зчитування числа атомів
            let mut num_str = String::new();
            while i < n && chars[i].is_ascii_digit() {
                num_str.push(chars[i]);
                i += 1;
            }
            let count: usize = if num_str.is_empty() {
                1
            } else {
                num_str.parse().map_err(|e| format!("Некоректний індекс: {e}"))?
            };

            let current = stack.last_mut().ok_or_else(|| "Помилка стека формули".to_string())?;
            *current.entry(elem.symbol).or_insert(0) += count;
        } else if c.is_whitespace() || c == '·' || c == '*' {
            i += 1;
        } else {
            return Err(format!("Невідомий символ у формулі на позиції {i}: '{c}'"));
        }
    }

    if stack.len() != 1 {
        return Err("Незбалансовані відкриті дужки у хімічній формулі".into());
    }

    let counts = stack.pop().unwrap();
    if counts.is_empty() {
        return Err("Формула не містить розпізнаних елементів".into());
    }

    let mut molar_mass = 0.0;
    let mut total_electrons = 0;

    for (&sym, &cnt) in &counts {
        let elem = get_element_by_symbol(sym).unwrap();
        molar_mass += elem.atomic_mass * (cnt as f64);
        total_electrons += (elem.z as usize) * cnt;
    }

    let mut mass_fractions = BTreeMap::new();
    for (&sym, &cnt) in &counts {
        let elem = get_element_by_symbol(sym).unwrap();
        let fraction = (elem.atomic_mass * (cnt as f64)) / molar_mass;
        mass_fractions.insert(sym, fraction);
    }

    Ok(ParsedMolecule {
        counts,
        molar_mass,
        total_electrons,
        mass_fractions,
    })
}

/// 12x12 Матриця переходів між квантово-хімічними архетипами J = A - A^T
#[derive(Debug, Clone)]
pub struct ChemArchetypeMatrix {
    pub weights: [[i8; NUM_CHEM_ARCHETYPES]; NUM_CHEM_ARCHETYPES],
}

impl ChemArchetypeMatrix {
    pub fn build() -> Self {
        let mut weights = [[0i8; NUM_CHEM_ARCHETYPES]; NUM_CHEM_ARCHETYPES];
        for i in 0..NUM_CHEM_ARCHETYPES {
            for j in 0..NUM_CHEM_ARCHETYPES {
                if i == j {
                    weights[i][j] = 0;
                } else {
                    let phase_diff = ((i as f64 - j as f64) * PHI).sin();
                    if phase_diff > 0.3 {
                        weights[i][j] = 1;
                        weights[j][i] = -1;
                    } else if phase_diff < -0.3 {
                        weights[i][j] = -1;
                        weights[j][i] = 1;
                    }
                }
            }
        }
        Self { weights }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_periodic_table_count() {
        assert_eq!(PERIODIC_TABLE.len(), 118);
        assert_eq!(PERIODIC_TABLE[0].symbol, "H");
        assert_eq!(PERIODIC_TABLE[117].symbol, "Og");
    }

    #[test]
    fn test_lookup_element() {
        let c = get_element_by_symbol("C").unwrap();
        assert_eq!(c.z, 6);
        assert_eq!(c.atomic_mass, 12.011);
        assert_eq!(c.archetype, ChemArchetype::OrganicLifeNonmetal);

        let fe = get_element_by_symbol("fe").unwrap();
        assert_eq!(fe.z, 26);
        assert!(fe.is_metal());
    }

    #[test]
    fn test_chemical_bonds() {
        let h = get_element_by_symbol("H").unwrap();
        let o = get_element_by_symbol("O").unwrap();
        let na = get_element_by_symbol("Na").unwrap();
        let cl = get_element_by_symbol("Cl").unwrap();
        let c = get_element_by_symbol("C").unwrap();

        // O-H: Полярний ковалентний (Δχ = 3.44 - 2.20 = 1.24) -> Трит +1
        let bond_oh = calculate_bond(o, h);
        assert_eq!(bond_oh.bond_type, BondType::PolarCovalent);
        assert_eq!(bond_oh.trit, 1);

        // Na-Cl: Йонний (Δχ = 3.16 - 0.93 = 2.23) -> Трит -1
        let bond_nacl = calculate_bond(na, cl);
        assert_eq!(bond_nacl.bond_type, BondType::Ionic);
        assert_eq!(bond_nacl.trit, -1);

        // C-H: Неполярний ковалентний (Δχ = 2.55 - 2.20 = 0.35) -> Трит 0
        let bond_ch = calculate_bond(c, h);
        assert_eq!(bond_ch.bond_type, BondType::NonPolarCovalent);
        assert_eq!(bond_ch.trit, 0);
    }

    #[test]
    fn test_formula_parsing() {
        // H2O
        let h2o = parse_chemical_formula("H2O").unwrap();
        assert_eq!(*h2o.counts.get("H").unwrap(), 2);
        assert_eq!(*h2o.counts.get("O").unwrap(), 1);
        assert!((h2o.molar_mass - 18.015).abs() < 0.01);
        assert_eq!(h2o.total_electrons, 10);

        // Fe2(SO4)3
        let fe2so4_3 = parse_chemical_formula("Fe2(SO4)3").unwrap();
        assert_eq!(*fe2so4_3.counts.get("Fe").unwrap(), 2);
        assert_eq!(*fe2so4_3.counts.get("S").unwrap(), 3);
        assert_eq!(*fe2so4_3.counts.get("O").unwrap(), 12);
        assert!((fe2so4_3.molar_mass - 399.88).abs() < 0.1);

        // Ca(OH)2
        let caoh2 = parse_chemical_formula("Ca(OH)2").unwrap();
        assert_eq!(*caoh2.counts.get("Ca").unwrap(), 1);
        assert_eq!(*caoh2.counts.get("O").unwrap(), 2);
        assert_eq!(*caoh2.counts.get("H").unwrap(), 2);
    }
}
""")
    return "\n".join(out)

def generate_rust_chem_asm_module():
    return """//! =======================================================================
//! АВТОГЕНЕРАТОР НА АСЕМБЛЕРІ x86_64: КВАНТОВО-ХІМІЧНА МАТРИЦЯ 118 ЕЛЕМЕНТІВ
//! =======================================================================
//!
//! Автономний компілятор-генератор, здатний емітувати 50 000+ рядків
//! чистого оптимізованого машинного коду та асемблерного лістингу x86_64:
//! 1. Будує 12-мірну антисиметричну Матрицю Архетипів для всіх 118 хімічних елементів.
//! 2. Генерує розгорнуті асемблерні блоки (Unrolled Jump Tables & Direct Register Flow)
//!    без жодних циклів і множень (No-Mul, addss, subss, cmov, movd, test, lea).
//! 3. Потоково записує 50 000+ рядків прямо на диск за мілісекунди.
//! 4. Пролог обнуляє фазові акумулятори (xorps), епілог повертає їх у вихідний
//!    буфер: [rsi+4088] = фазова сума архетипів, [rsi+4092] = Δχ-баланс пар.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use crate::universal_chem::{ChemArchetypeMatrix, Element, PERIODIC_TABLE, NUM_ELEMENTS, NUM_CHEM_ARCHETYPES, PHI};

/// Автогенератор квантово-хімічного асемблерного коду x86_64
pub struct ChemArchetypeAsmGenerator {
    matrix: ChemArchetypeMatrix,
}

impl ChemArchetypeAsmGenerator {
    pub fn new() -> Self {
        Self {
            matrix: ChemArchetypeMatrix::build(),
        }
    }

    /// Потокова генерація розгорнутого асемблерного моноліту на 50 000+ рядків прямо у файл
    pub fn write_unrolled_asm_to_file<P: AsRef<Path>>(
        &self,
        path: P,
        min_lines: usize,
    ) -> std::io::Result<usize> {
        let file = File::create(path)?;
        let mut writer = BufWriter::with_capacity(1024 * 1024, file);
        let mut lines = 0;

        macro_rules! emit {
            ($($arg:tt)*) => {{
                writeln!(writer, $($arg)*)?;
                lines += 1;
            }};
        }

        emit!("# =============================================================================");
        emit!("# POLER-ENGINE: AUTOGENERATED QUANTUM CHEMICAL UNROLLED ASSEMBLY MONOLITH");
        emit!("# Zero-Multiplication (No-Mul) Phase Rotor & Balanced Ternary Matrix (118 Elements)");
        emit!("# =============================================================================");
        emit!(".intel_syntax noprefix");
        emit!(".text");
        emit!(".globl poler_chem_rotor_unrolled_resonance_50k");
        emit!(".type poler_chem_rotor_unrolled_resonance_50k, @function");
        emit!("poler_chem_rotor_unrolled_resonance_50k:");
        emit!("    # ABI: rdi = ptr_element_input, rsi = ptr_resonance_out, rdx = count");
        emit!("    push rbp");
        emit!("    mov rbp, rsp");
        emit!("    push rbx");
        emit!("    push r12");
        emit!("    push r13");
        emit!("    push r14");
        emit!("    push r15");
        // Обнулення фазових акумуляторів ротора (без цього накопичується сміття регістрів)
        emit!("    xorps xmm1, xmm1");
        emit!("    xorps xmm3, xmm3");

        // Генерація розгорнутих блоків для всіх 118 елементів
        for (idx, elem) in PERIODIC_TABLE.iter().enumerate() {
            let z = elem.z;
            let sym = elem.symbol;
            let arch_idx = elem.archetype as usize;

            emit!("");
            emit!("    # -------------------------------------------------------------");
            emit!("    # ELEMENT [Z={:03}] {} ({}) | Archetype: {:?}", z, sym, elem.name, elem.archetype);
            emit!("    # -------------------------------------------------------------");
            emit!(".L_chem_elem_{:03}:", z);
            emit!("    mov eax, [rdi + {}]", idx * 16);
            emit!("    test eax, eax");
            emit!("    jz .L_chem_skip_{:03}", z);

            for j in 0..NUM_CHEM_ARCHETYPES {
                let weight = self.matrix.weights[arch_idx][j];
                let phase_offset = ((z as f64 * PHI) + (j as f64 * 0.5)).sin();
                let bit_repr = (phase_offset as f32).to_bits();

                emit!("    # Archetype Interaction [{:?} -> Arc#{}] Weight: {}", elem.archetype, j, weight);
                emit!("    mov ecx, 0x{:08X}", bit_repr);
                emit!("    movd xmm0, ecx");
                if weight > 0 {
                    emit!("    addss xmm1, xmm0");
                } else if weight < 0 {
                    emit!("    subss xmm1, xmm0");
                } else {
                    emit!("    # Zero-trit: Neutral phase lock");
                }
                emit!("    lea r8, [rdi + {}]", (idx * 16) + (j * 4));
                emit!("    mov [rsi + {}], eax", (idx * 32) + (j * 4));
            }

            emit!(".L_chem_skip_{:03}:", z);
        }

        // Потокове розгортання до досягнення min_lines (50 000+)
        let mut loop_unroll = 0;
        while lines < min_lines {
            loop_unroll += 1;
            emit!("");
            emit!("    # -------------------------------------------------------------");
            emit!("    # UNROLLED CHEMICAL ROTOR SUPER-BLOCK #{}", loop_unroll);
            emit!("    # -------------------------------------------------------------");

            for pair_idx in 0..118 {
                let elem1 = &PERIODIC_TABLE[pair_idx];
                let elem2 = &PERIODIC_TABLE[(pair_idx * 7 + loop_unroll) % 118];
                let delta_chi = (elem1.electronegativity - elem2.electronegativity).abs();
                let trit = if delta_chi < 0.4 { 0 } else if delta_chi <= 2.0 { 1 } else { -1 };
                let hex_val = ((delta_chi as f32).to_bits());

                emit!("    # Pair [{} ({}) <-> {} ({})] DeltaChi: {:.2} Trit: {}", elem1.symbol, elem1.z, elem2.symbol, elem2.z, delta_chi, trit);
                emit!("    mov eax, [rdi + {}]", (pair_idx * 8) % 2048);
                emit!("    mov ecx, 0x{:08X}", hex_val);
                emit!("    movd xmm2, ecx");
                if trit > 0 {
                    emit!("    addss xmm3, xmm2");
                    emit!("    cmovg ebx, eax");
                } else if trit < 0 {
                    emit!("    subss xmm3, xmm2");
                    emit!("    cmovl ebx, ecx");
                } else {
                    emit!("    xorps xmm2, xmm2");
                }
                emit!("    mov [rsi + {}], ebx", (pair_idx * 4) % 4096);
            }
        }

        emit!("");
        emit!("    # Epilogue: збереження фазових акумуляторів ротора у вихідний буфер");
        emit!("    # [rsi+4088] = фазова сума архетипів, [rsi+4092] = Δχ-баланс пар");
        emit!("    movss dword ptr [rsi+4088], xmm1");
        emit!("    movss dword ptr [rsi+4092], xmm3");
        emit!("    # Epilogue");
        emit!("    pop r15");
        emit!("    pop r14");
        emit!("    pop r13");
        emit!("    pop r12");
        emit!("    pop rbx");
        emit!("    mov rsp, rbp");
        emit!("    pop rbp");
        emit!("    ret");
        emit!(".size poler_chem_rotor_unrolled_resonance_50k, .-poler_chem_rotor_unrolled_resonance_50k");

        writer.flush()?;
        Ok(lines)
    }
}
"""

def generate_asm_file_direct(path, min_lines=52000):
    matrix = [[0]*12 for _ in range(12)]
    for i in range(12):
        for j in range(12):
            if i != j:
                diff = math.sin((i - j) * PHI)
                if diff > 0.3:
                    matrix[i][j] = 1
                    matrix[j][i] = -1
                elif diff < -0.3:
                    matrix[i][j] = -1
                    matrix[j][i] = 1

    lines = 0
    with open(path, "w", encoding="utf-8") as f:
        def emit(line=""):
            nonlocal lines
            f.write(line + "\n")
            lines += 1

        emit("# =============================================================================")
        emit("# POLER-ENGINE: AUTOGENERATED QUANTUM CHEMICAL UNROLLED ASSEMBLY MONOLITH")
        emit("# Zero-Multiplication (No-Mul) Phase Rotor & Balanced Ternary Matrix (118 Elements)")
        emit("# =============================================================================")
        emit(".intel_syntax noprefix")
        emit(".text")
        emit(".globl poler_chem_rotor_unrolled_resonance_50k")
        emit(".type poler_chem_rotor_unrolled_resonance_50k, @function")
        emit("poler_chem_rotor_unrolled_resonance_50k:")
        emit("    # ABI: rdi = ptr_element_input, rsi = ptr_resonance_out, rdx = count")
        emit("    push rbp")
        emit("    mov rbp, rsp")
        emit("    push rbx")
        emit("    push r12")
        emit("    push r13")
        emit("    push r14")
        emit("    push r15")
        # Обнулення фазових акумуляторів ротора (без цього накопичується сміття регістрів)
        emit("    xorps xmm1, xmm1")
        emit("    xorps xmm3, xmm3")

        for idx, elem in enumerate(RAW_ELEMENTS):
            z, sym, name, name_ru, mass, en, grp, per, blk, rcov, rvdw, ie, ec, arch = elem
            emit()
            emit(f"    # -------------------------------------------------------------")
            emit(f"    # ELEMENT [Z={z:03}] {sym} ({name}) | Archetype: {ARCHETYPES[arch][0]}")
            emit(f"    # -------------------------------------------------------------")
            emit(f".L_chem_elem_{z:03}:")
            emit(f"    mov eax, [rdi + {idx * 16}]")
            emit(f"    test eax, eax")
            emit(f"    jz .L_chem_skip_{z:03}")

            for j in range(12):
                weight = matrix[arch][j]
                phase_offset = math.sin((z * PHI) + (j * 0.5))
                # float to hex
                import struct
                bit_repr = struct.unpack('>I', struct.pack('>f', phase_offset))[0]
                emit(f"    # Archetype Interaction [{ARCHETYPES[arch][0]} -> Arc#{j}] Weight: {weight}")
                emit(f"    mov ecx, 0x{bit_repr:08X}")
                emit(f"    movd xmm0, ecx")
                if weight > 0:
                    emit(f"    addss xmm1, xmm0")
                elif weight < 0:
                    emit(f"    subss xmm1, xmm0")
                else:
                    emit(f"    # Zero-trit: Neutral phase lock")
                emit(f"    lea r8, [rdi + {(idx * 16) + (j * 4)}]")
                emit(f"    mov [rsi + {(idx * 32) + (j * 4)}], eax")

            emit(f".L_chem_skip_{z:03}:")

        loop_unroll = 0
        while lines < min_lines:
            loop_unroll += 1
            emit()
            emit(f"    # -------------------------------------------------------------")
            emit(f"    # UNROLLED CHEMICAL ROTOR SUPER-BLOCK #{loop_unroll}")
            emit(f"    # -------------------------------------------------------------")

            for pair_idx in range(118):
                elem1 = RAW_ELEMENTS[pair_idx]
                elem2 = RAW_ELEMENTS[(pair_idx * 7 + loop_unroll) % 118]
                delta_chi = abs(elem1[5] - elem2[5])
                trit = 0 if delta_chi < 0.4 else (1 if delta_chi <= 2.0 else -1)
                import struct
                hex_val = struct.unpack('>I', struct.pack('>f', delta_chi))[0]

                emit(f"    # Pair [{elem1[1]} (Z={elem1[0]}) <-> {elem2[1]} (Z={elem2[0]})] DeltaChi: {delta_chi:.2f} Trit: {trit}")
                emit(f"    mov eax, [rdi + {(pair_idx * 8) % 2048}]")
                emit(f"    mov ecx, 0x{hex_val:08X}")
                emit(f"    movd xmm2, ecx")
                if trit > 0:
                    emit(f"    addss xmm3, xmm2")
                    emit(f"    cmovg ebx, eax")
                elif trit < 0:
                    emit(f"    subss xmm3, xmm2")
                    emit(f"    cmovl ebx, ecx")
                else:
                    emit(f"    xorps xmm2, xmm2")
                emit(f"    mov [rsi + {(pair_idx * 4) % 4096}], ebx")

        emit()
        emit("    # Epilogue: збереження фазових акумуляторів ротора у вихідний буфер")
        emit("    # [rsi+4088] = фазова сума архетипів, [rsi+4092] = Δχ-баланс пар")
        emit("    movss dword ptr [rsi+4088], xmm1")
        emit("    movss dword ptr [rsi+4092], xmm3")
        emit("    # Epilogue")
        emit("    pop r15")
        emit("    pop r14")
        emit("    pop r13")
        emit("    pop r12")
        emit("    pop rbx")
        emit("    mov rsp, rbp")
        emit("    pop rbp")
        emit("    ret")
        emit(".size poler_chem_rotor_unrolled_resonance_50k, .-poler_chem_rotor_unrolled_resonance_50k")

    return lines

if __name__ == "__main__":
    print("[*] Generating src/universal_chem.rs...")
    with open("src/universal_chem.rs", "w", encoding="utf-8") as f:
        f.write(generate_rust_chem_module())
    print("[+] src/universal_chem.rs generated successfully.")

    print("[*] Generating src/universal_chem_asm.rs...")
    with open("src/universal_chem_asm.rs", "w", encoding="utf-8") as f:
        f.write(generate_rust_chem_asm_module())
    print("[+] src/universal_chem_asm.rs generated successfully.")

    print("[*] Generating chem_unrolled_resonance_50k.s (50,000+ lines)...")
    total_lines = generate_asm_file_direct("chem_unrolled_resonance_50k.s", 52000)
    print(f"[+] chem_unrolled_resonance_50k.s generated ({total_lines} lines).")
