// Copyright (c) 2020-2021 Jan Haller. zlib/libpng license.

function triple(a) {
    console.log("triple(" + a + ")");
    return 3 * a;
}

function extract(obj) {
    console.log("extract(" + obj + ")");

    return {
        new_text: obj.text + ".",
        new_num: triple(obj.num)
    };
}