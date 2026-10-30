// STDP упрощён до корреляции
delta = (pre_act * post_act) > 0 ? 0.01f : -0.005f;
