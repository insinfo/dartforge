
  call void @dartforge_memoria_arc_v1()
  %df.arc.abi = call i8 @dartforge_arc_verificar_abi(i64 1)
  %df.arc.abi.ok = icmp eq i8 %df.arc.abi, 1
  br i1 %df.arc.abi.ok, label %df.arc.compativel, label %df.arc.incompativel
df.arc.incompativel:
  call void @llvm.trap()
  unreachable
df.arc.compativel:
