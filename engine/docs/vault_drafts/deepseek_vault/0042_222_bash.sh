# 1. 下载源代码
wget https://www.gnu.org/software/xorriso/xorriso-1.4.6.tar.gz[reference:6][reference:7]

# 2. 解压并进入目录
tar xzf xorriso-1.4.6.tar.gz
cd xorriso-1.4.6

# 3. (可选) 避免可能出现的 'makeinfo' 错误
touch xorriso/*.info[reference:8][reference:9]

# 4. 配置并编译
./configure && make[reference:10][reference:11]
✅ 第三步：验证和使用

编译成功后，可以测试一下生成的二进制文件：

bash
