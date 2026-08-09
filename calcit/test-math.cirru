
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-math) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-math.main/main!) (:mode :native) (:reload-fn 'test-math.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-math.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing math") (test-math) (test-hex) (test-methods) (do true)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-hex $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing hex") (assert= 16 0x10) (assert= 15 0xf)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-math $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-math ()
              println "|sin 1" $ sin 1
              println "|cos 1" $ cos 1
              assert= 1 $ +
                pow (sin 1) 2
                pow (cos 1) 2
              assert= 1 $ floor 1.1
              assert= 2 $ ceil 1.1
              assert= 1 $ round 1.1
              assert= 2 $ round 1.8
              assert= 2 $ .round 1.8
              assert= 0.8 $ .fract 1.8
              assert= 81 $ pow 3 4
              assert= 1 $ &number:rem 33 4
              assert= 9 $ sqrt 81
              println |PI &PI
              println |E &E
              assert= 4 $ abs 4
              assert= (negate -4) (abs -4)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing number methods")
              assert= 1 $ .floor 1.1
              assert= 16 $ .pow 2 4
              assert= 2 $ .ceil 1.1
              assert= 0 $ .empty 1.1
              assert= 2.1 $ .inc 1.1
              assert= 1 $ .round 1.1
              assert= false $ .round? 1.1
              assert= true $ .round? 1
              assert= 2 $ .sqrt 4
              assert= 3 $ .rem 3 6
              assert= 2 $ .rem 11 3
              ; "has problem in comparing float numbers" $ assert= 0.1 (.fract 1.1)
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-math.main $ :require
            util.core :refer $ log-title
