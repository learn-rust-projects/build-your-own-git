# 测试

## 可执行权限

```shell
chmod -R +x ../
```

## 测试 init ls_tree cat_file

```shell
./run_tests.sh
```


## Git want - have 测试

### 测试want - have

Step 1：发送请求并把返回内容存成 packfile

```shell
curl -v \
  -o packfile.bin1 \
  -X POST \
  -H "Content-Type: application/x-git-upload-pack-request" \
  --data-binary $'0032want 9671f5a72cac8b4c379b1c35a6af6d10611d620f\n00000009done\n' \
  "https://github.com/learn-rust-projects/build-your-own-git/git-upload-pack"
```

--data-binary：发送二进制内容，不做任何转义/编码。

-o packfile.bin： 把服务器返回的 packfile 保存为本地文件。

Git 的 pkt-line 不要求你发送能力列表（capabilities），最小实现只要 want + done 即可启动 packfile 输出。


## 测试其他命令

```shell
./your_program.sh init
# 添加文件
./your_program.sh commit -m "xxx"
# 测试 clone
./your_program.sh clone https://github.com/learn-rust-projects/build-your-own-git.git
```