# 测试

## 准备

### 为测试文件添加可执行权限以及编译可执行文件

```shell
find ../ -type f -name "*.sh" -exec chmod +x {} \;
```

## 运行

### 运行所有测试

```shell
make test
```

### 测试其他命令

```shell
make install
mkdir test-repo
cd test-repo
own-git init
echo "hello world" > test.txt
own-git commit -m "init"
make uninstall
```


### Git want - have 测试

#### 直接发送请求并把返回内容存成 packfile

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

#### 先创建文件再发送请求

##### 测试want - have

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