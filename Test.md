# 测试

## Git want - have 测试

### 测试want - have

Step 1：创建文件：`upload-pack-req.txt`

```shell
cat > upload-pack-req.txt <<EOF
0032want 9671f5a72cac8b4c379b1c35a6af6d10611d620f
00000009done
EOF

```

Git 的 pkt-line 不要求你发送能力列表（capabilities），最小实现只要 want + done 即可启动 packfile 输出。

Step 2：发送请求并把返回内容存成 packfile

```shell
curl -v \
  -o packfile.bin\
  -X POST \
  -H "Content-Type: application/x-git-upload-pack-request" \
  --data-binary "@upload-pack-req.txt" \
  "https://github.com/learn-rust-projects/build-your-own-git/git-upload-pack" 

```

--data-binary：发送二进制内容，不做任何转义/编码。

-o packfile.bin： 把服务器返回的 packfile 保存为本地文件。