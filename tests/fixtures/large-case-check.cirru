
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'chain0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain0 (name) (lookup name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain1 (name) (chain0 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain10 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain10 (name) (chain9 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain11 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain11 (name) (chain10 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain12 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain12 (name) (chain11 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain13 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain13 (name) (chain12 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain14 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain14 (name) (chain13 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain15 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain15 (name) (chain14 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain16 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain16 (name) (chain15 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain17 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain17 (name) (chain16 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain18 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain18 (name) (chain17 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain19 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain19 (name) (chain18 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain2 (name) (chain1 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain3 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain3 (name) (chain2 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain4 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain4 (name) (chain3 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain5 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain5 (name) (chain4 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain6 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain6 (name) (chain5 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain7 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain7 (name) (chain6 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain8 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain8 (name) (chain7 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'chain9 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn chain9 (name) (chain8 name)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
        'lookup $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lookup (name)
            case-default name -1 (|p0 0) (|p1 1) (|p2 2) (|p3 3) (|p4 4) (|p5 5) (|p6 6) (|p7 7) (|p8 8) (|p9 9) (|p10 10) (|p11 11) (|p12 12) (|p13 13) (|p14 14) (|p15 15) (|p16 16) (|p17 17) (|p18 18) (|p19 19) (|p20 20) (|p21 21) (|p22 22) (|p23 23) (|p24 24) (|p25 25) (|p26 26) (|p27 27) (|p28 28) (|p29 29) (|p30 30) (|p31 31) (|p32 32) (|p33 33) (|p34 34) (|p35 35) (|p36 36) (|p37 37) (|p38 38) (|p39 39) (|p40 40) (|p41 41) (|p42 42) (|p43 43) (|p44 44) (|p45 45) (|p46 46) (|p47 47) (|p48 48) (|p49 49) (|p50 50) (|p51 51) (|p52 52) (|p53 53) (|p54 54) (|p55 55) (|p56 56) (|p57 57) (|p58 58) (|p59 59) (|p60 60) (|p61 61) (|p62 62) (|p63 63) (|p64 64) (|p65 65) (|p66 66) (|p67 67) (|p68 68) (|p69 69) (|p70 70) (|p71 71) (|p72 72) (|p73 73) (|p74 74) (|p75 75) (|p76 76) (|p77 77) (|p78 78) (|p79 79) (|p80 80) (|p81 81) (|p82 82) (|p83 83) (|p84 84) (|p85 85) (|p86 86) (|p87 87) (|p88 88) (|p89 89) (|p90 90) (|p91 91) (|p92 92) (|p93 93) (|p94 94) (|p95 95) (|p96 96) (|p97 97) (|p98 98) (|p99 99) (|p100 100) (|p101 101) (|p102 102) (|p103 103) (|p104 104) (|p105 105) (|p106 106) (|p107 107) (|p108 108) (|p109 109) (|p110 110) (|p111 111) (|p112 112) (|p113 113) (|p114 114) (|p115 115) (|p116 116) (|p117 117) (|p118 118) (|p119 119) (|p120 120) (|p121 121) (|p122 122) (|p123 123) (|p124 124) (|p125 125) (|p126 126) (|p127 127) (|p128 128) (|p129 129) (|p130 130) (|p131 131) (|p132 132) (|p133 133) (|p134 134) (|p135 135) (|p136 136) (|p137 137) (|p138 138) (|p139 139) (|p140 140) (|p141 141) (|p142 142) (|p143 143) (|p144 144) (|p145 145) (|p146 146) (|p147 147) (|p148 148) (|p149 149) (|p150 150) (|p151 151) (|p152 152) (|p153 153) (|p154 154) (|p155 155) (|p156 156) (|p157 157) (|p158 158) (|p159 159)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |large-case-branches)
            :code $ quote $ do
              assert= (lookup |p0) 0
              assert= (lookup |p79) 79
              assert= (lookup |p159) 159
              assert= (lookup |missing) -1
            :tags $ #{} :core :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (chain19 |p79)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
