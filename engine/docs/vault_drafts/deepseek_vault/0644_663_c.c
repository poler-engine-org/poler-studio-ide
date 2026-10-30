static void stdp_update_real(LanguageCore* lang, float reward) {
for (int k = 0; k < lang->base.link_count; k++) {
Link* ln = &lang->base.links[k];

// Когда был спайк пре-нейрона?
int pre_time = -1;
for (int t = 0; t < TRACE_WINDOW; t++) {
if (lang->plast.pre_trace[ln->src][t] > 0) {
pre_time = t;
break;
}
}

// Когда был спайк пост-нейрона?
int post_time = -1;
for (int t = 0; t < TRACE_WINDOW; t++) {
if (lang->plast.pre_trace[ln->dst][t] > 0) {
post_time = t;
break;
}
}

if (pre_time >= 0 && post_time >= 0) {
float dt = post_time - pre_time;  // время между спайками
float delta;

if (dt > 0) {
// пре раньше пост → LTP
delta = 0.01f * expf(-dt / 10.0f);
} else {
// пост раньше пре → LTD
delta = -0.005f * expf(dt / 10.0f);
}

lang->plast.eligibility[k] =
0.9f * lang->plast.eligibility[k] + delta;

ln->weight += lang->plast.learning_rate * reward * lang->plast.eligibility[k];
}
}
}
