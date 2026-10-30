#include <iostream>
#include <bitset>
#include <string>
using namespace std;

int main() {
string bits = "11110010010110011100110101011100"; // 32 бита
unsigned int num = stoul(bits, nullptr, 2);
float fnum;
memcpy(&fnum, &num, sizeof(float));

cout << "Hex: 0x" << hex << num << endl;
cout << "Unsigned dec: " << dec << num << endl;
cout << "Signed dec: " << (int)num << endl;
cout << "Float: " << fnum << endl;

// Переходы
    cout << "Переходы (индексы начала нового бита): ";
for (int i = 1; i < bits.size(); ++i) {
if (bits[i] != bits[i-1])
cout << i << " ";
}
cout << endl;

return 0;
}
